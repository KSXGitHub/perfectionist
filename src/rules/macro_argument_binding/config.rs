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

/// Macros known to evaluate every top-level argument exactly once,
/// plus the curated set of `core` / `std` macros that operate purely
/// at compile time (`concat!`, `env!`, `include_str!`, ...): their
/// arguments are either literals or other compile-time-pure macro
/// calls, never runtime expressions whose evaluation order matters.
/// The list mirrors the curated set in `macro_trailing_comma`, with
/// the conditional-evaluation families (`log::*`, `tracing::*`)
/// removed because those *do* drop arguments below the configured
/// filter level.
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
    // `core` / `std` compile-time macros. Each accepts only literals,
    // identifiers, or other compile-time macro calls (and `is_x86_*` /
    // `cfg!` accept only literal-shaped feature / cfg predicates).
    // There is no observable evaluation order to disturb, so passing
    // any expression — even a nested macro that itself produces a
    // literal — is safe.
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
];

/// `core` / `std` macros whose invocation expands to a value the
/// compiler computes at build time — a literal, a `&'static str`, a
/// byte string, a `bool` cfg verdict, a line / column / file marker.
/// None evaluates a runtime expression, none has side effects, so an
/// `inner!(...)` call to one of these is itself a trivial argument
/// for the surrounding macro: it cannot be evaluated more than once
/// at runtime no matter what the outer macro does with it.
///
/// `include!` is deliberately excluded — its expansion is arbitrary
/// Rust code rather than a literal, so its triviality depends on the
/// included file's contents and the rule cannot prove it.
/// `compile_error!` is also excluded: its expansion is the diverging
/// `!` type rather than a value, and the planning doc reserves the
/// trivial-atom slot for value-producing macros.
const BUILTIN_TRIVIAL_MACROS: &[&str] = &[
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
/// many times the macro touches them, so they are accepted as trivial
/// postfixes on a trivial base. Names whose pure-getter convention is
/// less universal (e.g. `count` is consuming on `Iterator` but
/// `O(1)` and pure on indexed collections) are left for projects to
/// add via `extra_trivial_methods`.
const BUILTIN_TRIVIAL_METHODS: &[&str] = &[
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
    /// a non-trivial top-level argument, regardless of any built-in
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
    /// `.method()` invocation on a trivial base is then accepted as a
    /// trivial postfix when the method takes no arguments.
    pub extra_trivial_methods: Vec<String>,
    /// Method names to drop from the pure-method list, even if they
    /// appear in the built-in defaults or in `extra_trivial_methods`.
    /// Empty by default; checked after the merge, so this knob always
    /// wins. Useful for opting back into linting on a default entry
    /// the project does not consider trivial — for example, removing
    /// `as_ref` for a project that wraps it in a non-pure
    /// implementation.
    pub ignore_trivial_methods: Vec<String>,
    /// Macro names added to the built-in trivial-macro list. Each
    /// entry is matched against the invocation's final path segment
    /// (so `my_crate::const_str` matches by the `"const_str"` tail).
    /// A trivial-macro call passed as an argument to another macro is
    /// treated as a trivial atom — the rule does not propose binding
    /// it to a `let`. Use this knob for project-specific macros whose
    /// expansion is guaranteed to be a literal or other compile-time
    /// constant.
    pub extra_trivial_macros: Vec<String>,
    /// Macro names to drop from the trivial-macro list, even if they
    /// appear in the built-in defaults or in `extra_trivial_macros`.
    /// Checked after the merge, so this knob always wins.
    pub ignore_trivial_macros: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: Mode::default(),
            deny_extra: Vec::new(),
            allow_extra: Vec::new(),
            ignore: Vec::new(),
            extra_trivial_methods: Vec::new(),
            ignore_trivial_methods: Vec::new(),
            extra_trivial_macros: Vec::new(),
            ignore_trivial_macros: Vec::new(),
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
    /// Built-in pure-method list plus `extra_trivial_methods`,
    /// consulted by the trivial-expression walker to accept
    /// `expr.method()` as a trivial postfix on a trivial base.
    trivial_methods: BTreeSet<String>,
    /// Built-in trivial-macro list plus `extra_trivial_macros`,
    /// consulted by the trivial-expression walker to accept
    /// `inner!(...)` as a trivial atom when the macro's expansion
    /// is a compile-time constant. Match is tail-segment-based:
    /// an entry of `"env"` accepts `env!(...)`, `std::env!(...)`,
    /// and `::core::env!(...)` alike.
    trivial_macros: BTreeSet<String>,
}

impl MacroArgumentBinding {
    pub(super) fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let extra_deny = parse_path_list(&config.deny_extra);
        let extra_allow = parse_path_list(&config.allow_extra);
        let deny = merge_with_builtins(BUILTIN_DENY, &extra_deny);
        let allow = merge_with_builtins(BUILTIN_ALLOW, &extra_allow);
        let ignore = parse_path_list(&config.ignore);
        let trivial_methods = merge_string_allowlist(
            BUILTIN_TRIVIAL_METHODS,
            config.extra_trivial_methods,
            config.ignore_trivial_methods,
        );
        let trivial_macros = merge_string_allowlist(
            BUILTIN_TRIVIAL_MACROS,
            config.extra_trivial_macros,
            config.ignore_trivial_macros,
        );
        Self {
            enabled: config.enabled,
            mode: config.mode,
            deny,
            allow,
            allow_extra: extra_allow,
            ignore,
            trivial_methods,
            trivial_macros,
        }
    }

    /// The merged set of method names whose `.method()` invocations
    /// on a trivial base are accepted as trivial postfixes.
    pub(super) fn trivial_methods(&self) -> &BTreeSet<String> {
        &self.trivial_methods
    }

    /// The merged set of macro names whose `inner!(...)` invocations
    /// are accepted as trivial atoms. Matched by final path segment,
    /// so a single-name entry covers fully-qualified call sites too.
    pub(super) fn trivial_macros(&self) -> &BTreeSet<String> {
        &self.trivial_macros
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
