//! Configuration for `macro_argument_binding`. Owns the `Mode` enum,
//! the user-facing `Config` shape, the curated built-in deny / allow
//! lists, and the in-memory `MacroArgumentBinding` state the early
//! pass holds.
//!
//! Path-set construction goes through the helpers at the bottom of
//! this file so each list (`deny`, `allow`, `allow_extra`, `ignore`)
//! is built consistently.

use std::collections::BTreeSet;

use rustc_ast::Path;

use crate::common::merge_string_allowlist;
use crate::macro_path::{matches_any, merge_with_builtins, parse_path_list};

const CONFIG_KEY: &str = "perfectionist::macro_argument_binding";

/// Macros whose argument list is checked unconditionally because the
/// expansion is known to evaluate captures conditionally on a `cfg`
/// (`debug_assert*`) or to drop them entirely in release builds.
const BUILTIN_DENY: &[&str] = &["debug_assert", "debug_assert_eq", "debug_assert_ne"];

/// Macros for which the call site is known not to introduce the
/// "evaluate zero, one, or many times" hazard. Three disjoint groups:
///
/// 1. Runtime macros (`format!`, `vec!`, `assert!`, `dbg!`, …) that
///    promise to evaluate every top-level argument exactly once.
///    Shares its core with `macro_trailing_comma`'s built-in set,
///    minus the conditional-evaluation families (`log::*`,
///    `tracing::*`) that *do* drop arguments below the configured
///    filter level.
/// 2. `core` / `std` macros whose top-level argument simply isn't a
///    runtime expression — `stringify!` takes a token sequence,
///    `cfg!` takes a cfg predicate, the `env!` / `include_*` /
///    `is_x86_feature_detected!` family takes a string literal, the
///    `line!` / `column!` / `file!` / `module_path!` family takes
///    no argument, `compile_error!` aborts compilation — or, in the
///    matryoshka case from issue #71, is itself a call to another
///    macro from this group. None of these evaluates a user
///    expression at runtime, so no exactly-once-vs.-zero hazard
///    surfaces at the call site.
/// 3. Third-party macros whose matchers are known to evaluate every
///    top-level argument exactly once before forwarding it, so the
///    once-vs.-zero hazard does not surface at the call site. New
///    entries land here as they're identified; the group is open-
///    ended and not tied to any single crate.
///
/// `macro_trailing_comma`'s own built-in list is intentionally
/// narrower than this one (it has no reason to care about
/// `stringify!` / `cfg!` / `include_*` etc.); the two lists are not
/// kept in lockstep.
const BUILTIN_ALLOW: &[&str] = &[
    // Runtime macros that promise exactly-once evaluation per argument.
    "format",
    "format_args",
    "print",
    "println",
    "eprint",
    "eprintln",
    "write",
    "writeln",
    "vec",
    "panic",
    "unimplemented",
    "todo",
    "unreachable",
    "assert",
    "assert_eq",
    "assert_ne",
    "matches",
    "dbg",
    "anyhow",
    // `core` / `std` macros that do not evaluate a runtime user
    // expression at the call site (see the doc comment above for
    // the per-macro rationale).
    "cfg",
    "column",
    "compile_error",
    "concat",
    "env",
    "file",
    "include",
    "include_bytes",
    "include_str",
    "is_x86_feature_detected",
    "line",
    "module_path",
    "option_env",
    "stringify",
    // Third-party macros whose matchers evaluate every top-level
    // argument exactly once. Entries are grouped by crate in the
    // commentary below; the array itself is alphabetised because
    // the matcher is tail-segment-keyed and doesn't care about
    // origin.
    //
    // - `anyhow::{bail, ensure}` are companions to `anyhow!` (in
    //   group 1). `bail!(args)` expands to `return Err(anyhow!(args))`
    //   and evaluates each captured expression once; `ensure!(cond,
    //   args)` matches the shape of `assert!(cond, args)` already on
    //   group 1 — `cond` always evaluates, `args` only on failure.
    // - `insta`'s snapshot-assertion family. Each variant
    //   evaluates its value argument exactly once before
    //   serialising; `assert_display_snapshot` is deprecated
    //   upstream but retained for projects on older `insta`;
    //   `assert_binary_snapshot` is the newer byte-slice variant.
    // - `maplit::{hashmap, btreemap, hashset, btreeset}` expand to
    //   one `.insert` call per pair; each captured key and value
    //   is evaluated once. These entries are functionally
    //   redundant given `looks_like_expression`'s top-level `=>`
    //   DSL-marker skip (every non-empty maplit call form emits
    //   `=>` at the top level of each argument and is skipped
    //   regardless); they're retained as an explicit declaration
    //   that the project has vetted these macros for the once-
    //   only contract.
    // - `serde_json::json` builds a `serde_json::Value` tree; each
    //   embedded Rust expression goes through `to_value(&expr)`
    //   exactly once. Unlike `maplit::*`, this entry is load-
    //   bearing for direct-expression arguments — `json!(expr)`
    //   has no DSL marker, so removing the entry would re-flag
    //   `json!(value().unwrap())` and similar.
    //
    // Add further crates here as their matchers are vetted to
    // honour the same once-only contract.
    "assert_binary_snapshot",
    "assert_compact_debug_snapshot",
    "assert_compact_json_snapshot",
    "assert_csv_snapshot",
    "assert_debug_snapshot",
    "assert_display_snapshot",
    "assert_json_snapshot",
    "assert_ron_snapshot",
    "assert_snapshot",
    "assert_toml_snapshot",
    "assert_yaml_snapshot",
    "bail",
    "btreemap",
    "btreeset",
    "ensure",
    "hashmap",
    "hashset",
    "json",
];

/// `core` / `std` macros whose invocation expands to a value the
/// compiler computes at build time — a literal, a `&'static str`, a
/// byte string, a `bool` cfg verdict, a line / column / file marker.
/// None evaluates a runtime expression, none has side effects, so an
/// `inner!(...)` call to one of these is itself a pure argument
/// for the surrounding macro: it cannot be evaluated more than once
/// at runtime no matter what the outer macro does with it.
///
/// `include!` is deliberately excluded — its expansion is arbitrary
/// Rust code rather than a literal, so its purity depends on the
/// included file's contents and the rule cannot prove it.
/// `compile_error!` is also excluded: its expansion is the diverging
/// `!` type rather than a value, and the planning doc reserves the
/// pure-atom slot for value-producing macros.
const BUILTIN_PURE_MACROS: &[&str] = &[
    "cfg",
    "column",
    "concat",
    "env",
    "file",
    "include_bytes",
    "include_str",
    "line",
    "module_path",
    "option_env",
    "stringify",
];

/// Zero-arg method names that are conventionally side-effect-free
/// across the standard library and ecosystem. `vec.len()`,
/// `s.is_empty()`, `opt.as_ref()` evaluate the same way no matter how
/// many times the macro touches them, so they are accepted as pure
/// postfixes on a pure base. Names whose pure-getter convention is
/// less universal (e.g. `count` is consuming on `Iterator` but
/// `O(1)` and pure on indexed collections) are left for projects to
/// add via `extra_pure_methods`.
const BUILTIN_PURE_METHODS: &[&str] = &[
    "as_bytes", "as_deref", "as_mut", "as_ref", "as_slice", "as_str", "is_empty", "len",
];

/// Eligibility mode. The default is `AllowAndDeny`. The matcher-based
/// mode described in `planned-rules/macro-argument-binding.md` is not
/// yet implemented and is therefore not exposed as a value here; a
/// `dylint.toml` that names it will fail to deserialise with a
/// useful error.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Mode {
    /// Flag only invocations of the curated deny list (`debug_assert*`
    /// plus `deny_extra`). Every other macro is silently accepted.
    DenyOnly,
    /// Flag every function-like or array-like invocation that carries
    /// an impure top-level argument, regardless of any built-in
    /// classification — unless the invocation matches an `allow_extra`
    /// entry. The built-in allow list is deliberately ignored in this
    /// mode; project exceptions go in `allow_extra`.
    Blanket,
    /// Curated deny list plus curated allow list, both extensible via
    /// `deny_extra` / `allow_extra`. Macros on neither list are
    /// flagged — flagging unrecognised macros is deliberate so the
    /// rule remains useful in projects that depend on uncatalogued
    /// proc macros.
    #[default]
    AllowAndDeny,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Master on/off switch for the rule. Defaults to `true`. Set
    /// to `false` to silence every diagnostic this lint would emit
    /// without having to enumerate every macro under `ignore`.
    pub enabled: bool,
    /// Eligibility mode.
    pub mode: Mode,
    /// Macros added to the built-in deny list. Each entry is a
    /// fully-qualified macro path (no trailing `!`) or a bare macro
    /// name to match by final segment only.
    pub deny_extra: Vec<String>,
    /// Macros added to the built-in allow list. Same matching rules
    /// as `deny_extra`. Only meaningful in `AllowAndDeny` and
    /// `Blanket` modes; in `DenyOnly` the allow list is unused.
    pub allow_extra: Vec<String>,
    /// Macros to skip entirely, regardless of which list they would
    /// otherwise hit. Same matching rules as `deny_extra`.
    pub ignore: Vec<String>,
    /// Method names added to the built-in pure-method list. Each
    /// entry is a bare method identifier (no `()`, no receiver). A
    /// `.method()` invocation on a pure base is then accepted as a
    /// pure postfix when the method takes no arguments. Add a
    /// project-local method here only when it is genuinely safe
    /// for the surrounding macro to drop or duplicate the call
    /// (the rule's working definition of *pure*) — typically an
    /// `O(1)` side-effect-free getter that the lint's syntactic
    /// classification can't otherwise see.
    pub extra_pure_methods: Vec<String>,
    /// Method names to drop from the pure-method list, even if they
    /// appear in the built-in defaults or in `extra_pure_methods`.
    /// Empty by default; checked after the merge, so this knob always
    /// wins. Useful for opting back into linting on a default entry
    /// the project does not consider pure — for example, removing
    /// `as_ref` for a project that wraps it in an impure
    /// implementation.
    pub ignore_pure_methods: Vec<String>,
    /// Macro names added to the built-in pure-macro list. Each
    /// entry is matched against the invocation's final path segment
    /// (so `my_crate::const_str` matches by the `"const_str"` tail).
    /// A pure-macro call passed as an argument to another macro is
    /// treated as a pure atom — the rule does not propose binding
    /// it to a `let`. Use this knob for project-specific macros
    /// whose expansion is a compile-time constant (a literal, a
    /// `&'static str`, a `bool`); their inclusion satisfies the
    /// rule's pure-as-drop-or-duplicate-safe definition trivially,
    /// since there is no runtime expression for the surrounding
    /// macro to drop or duplicate.
    pub extra_pure_macros: Vec<String>,
    /// Macro names to drop from the pure-macro list, even if they
    /// appear in the built-in defaults or in `extra_pure_macros`.
    /// Checked after the merge, so this knob always wins.
    pub ignore_pure_macros: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: Mode::default(),
            deny_extra: Vec::new(),
            allow_extra: Vec::new(),
            ignore: Vec::new(),
            extra_pure_methods: Vec::new(),
            ignore_pure_methods: Vec::new(),
            extra_pure_macros: Vec::new(),
            ignore_pure_macros: Vec::new(),
        }
    }
}

pub(super) struct MacroArgumentBinding {
    enabled: bool,
    mode: Mode,
    /// Built-in deny list plus `deny_extra`. Used in `DenyOnly` and
    /// `AllowAndDeny`.
    deny: BTreeSet<Vec<String>>,
    /// Built-in allow list plus `allow_extra`. Used only in
    /// `AllowAndDeny`; `Blanket` deliberately ignores the built-in
    /// allow list and consults `allow_extra` alone.
    allow: BTreeSet<Vec<String>>,
    /// Only the user-supplied `allow_extra` entries. Used in
    /// `Blanket` mode, which has no built-in allow list per the rule
    /// docs (`planned-rules/macro-argument-binding.md`).
    allow_extra: BTreeSet<Vec<String>>,
    /// Macros to skip entirely. Checked before deny / allow lookup, so
    /// an entry here wins over any other classification.
    ignore: BTreeSet<Vec<String>>,
    /// Built-in pure-method list plus `extra_pure_methods`,
    /// consulted by the pure-expression walker to accept
    /// `expr.method()` as a pure postfix on a pure base.
    pure_methods: BTreeSet<String>,
    /// Built-in pure-macro list plus `extra_pure_macros`,
    /// consulted by the pure-expression walker to accept
    /// `inner!(...)` as a pure atom when the macro's expansion
    /// is a compile-time constant. Match is tail-segment-based:
    /// an entry of `"env"` accepts `env!(...)`, `std::env!(...)`,
    /// and `::core::env!(...)` alike.
    pure_macros: BTreeSet<String>,
}

impl MacroArgumentBinding {
    pub(super) fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let extra_deny = parse_path_list(&config.deny_extra);
        let extra_allow = parse_path_list(&config.allow_extra);
        let deny = merge_with_builtins(BUILTIN_DENY, &extra_deny);
        let allow = merge_with_builtins(BUILTIN_ALLOW, &extra_allow);
        let ignore = parse_path_list(&config.ignore);
        let pure_methods = merge_string_allowlist(
            BUILTIN_PURE_METHODS,
            config.extra_pure_methods,
            config.ignore_pure_methods,
        );
        let pure_macros = merge_string_allowlist(
            BUILTIN_PURE_MACROS,
            config.extra_pure_macros,
            config.ignore_pure_macros,
        );
        Self {
            enabled: config.enabled,
            mode: config.mode,
            deny,
            allow,
            allow_extra: extra_allow,
            ignore,
            pure_methods,
            pure_macros,
        }
    }

    /// The merged set of method names whose `.method()` invocations
    /// on a pure base are accepted as pure postfixes.
    pub(super) fn pure_methods(&self) -> &BTreeSet<String> {
        &self.pure_methods
    }

    /// The merged set of macro names whose `inner!(...)` invocations
    /// are accepted as pure atoms. Matched by final path segment,
    /// so a single-name entry covers fully-qualified call sites too.
    pub(super) fn pure_macros(&self) -> &BTreeSet<String> {
        &self.pure_macros
    }

    /// Path-side eligibility: combines the `enabled` switch, the
    /// `ignore` list, and the mode-based deny / allow lookup. Does
    /// *not* consider the call's delimiter or argument shape — those
    /// stay in the early-pass driver, where token-tree concerns live.
    pub(super) fn should_check_path(&self, path: &Path) -> bool {
        self.enabled && !matches_any(path, &self.ignore) && self.arguments_should_be_checked(path)
    }

    fn arguments_should_be_checked(&self, path: &Path) -> bool {
        let on_deny = matches_any(path, &self.deny);
        match self.mode {
            Mode::DenyOnly => on_deny,
            Mode::Blanket => !matches_any(path, &self.allow_extra),
            Mode::AllowAndDeny => on_deny || !matches_any(path, &self.allow),
        }
    }
}
