//! Helpers shared between sibling rules.
//!
//! Each helper lives here only because more than one rule needs it.
//! Anything used by a single rule belongs in that rule's own file.

use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

use rustc_hir as hir;
use rustc_hir::HirId;
use rustc_lint::{LateContext, LintContext};
use rustc_span::{Span, Symbol};

/// Whether the HIR node at `hir_id` (whose own span is `span`)
/// originates in an external proc-macro (or `macro_rules!`)
/// expansion.
///
/// `declare_tool_lint!(... report_in_external_macro: false)` only
/// inspects the diagnostic span when deciding whether to suppress.
/// Proc-macro derives such as `clap_derive`'s `default_value_t`
/// expansion synthesise nodes whose identifier inherits a
/// user-source span (the span of the attribute that drove the
/// expansion) so that downstream compile errors point somewhere a
/// user can fix; from the lint's perspective the identifier looks
/// user-authored even though the surrounding statement only exists
/// in the expansion. Every rule whose diagnostic span is narrower
/// than the syntactic node that produced the violation must
/// therefore check the structural-parent span explicitly.
///
/// Two checks are needed because some structural spans cover only
/// the identifier itself (a `<T>` generic parameter has no other
/// tokens), so the node's own `Span::in_external_macro` returns
/// false. Walking up to the enclosing item and checking its
/// `def_span` catches that case — the synthesised owner item's
/// span carries the expansion's `SyntaxContext`. Regression
/// fixtures live in `ui/*_proc_macro.rs` with a minimal derive in
/// `ui/auxiliary/proc_macro_synth_binding.rs`.
pub(crate) fn hir_in_external_macro(cx: &LateContext<'_>, hir_id: HirId, span: Span) -> bool {
    let sm = cx.sess().source_map();
    if span.in_external_macro(sm) {
        return true;
    }
    let owner_id = cx.tcx.hir_get_parent_item(hir_id);
    cx.tcx.def_span(owner_id.to_def_id()).in_external_macro(sm)
}

/// Crate-wide configuration table, deserialised from the top-level
/// `[perfectionist]` table of `dylint.toml`. Each entry of `enable`
/// flips a rule that was off by default to on; each entry of
/// `disable` flips a rule that was on by default (the common case)
/// to off. The two arrays accept either a bare rule name (a string)
/// or an inline `{ name, reason }` table — the `reason` field is
/// decorative and ignored at runtime, present so config authors can
/// leave a rationale next to the entry for future readers without
/// hiding it in a TOML comment. Listing the same rule under both
/// arrays is a config error.
///
/// "Enable" / "disable" deliberately doesn't mention lint levels: it
/// toggles whether the rule's pass is installed at all. The lint
/// itself stays registered either way, so `#[expect/allow/deny(
/// perfectionist::<rule>)]` at the call site continues to resolve
/// against the registered lint set; users that want to escalate a
/// rule's level above `Warn` reach for `#![deny(perfectionist::
/// <rule>)]` or `DYLINT_RUSTFLAGS=-D perfectionist::<rule>` as
/// before — the only mechanism rustc actually exposes for level
/// changes from outside the source.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct GlobalConfig {
    enable: Vec<RuleSelector>,
    disable: Vec<RuleSelector>,
}

/// Each `enable` / `disable` entry deserialises from either a bare
/// string or an inline `{ name = "...", reason = "..." }` table.
/// `#[serde(untagged)]` is what makes the array mixable, so a
/// config author can write
/// `enable = ["a", { name = "b", reason = "rationale" }]` in a
/// single literal array.
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum RuleSelector {
    Name(String),
    Verbose {
        name: String,
        #[expect(
            dead_code,
            reason = "decorative field for human readers of dylint.toml"
        )]
        reason: Option<String>,
    },
}

impl RuleSelector {
    fn name(&self) -> &str {
        match self {
            RuleSelector::Name(name) | RuleSelector::Verbose { name, .. } => name,
        }
    }
}

/// Resolved per-rule override map. `true` means the user explicitly
/// listed the rule under `enable`; `false` means under `disable`.
/// Rules absent from this map fall through to the per-rule default
/// each `register_pass` declares.
static GLOBAL_OVERRIDES: OnceLock<HashMap<String, bool>> = OnceLock::new();

/// Parse the `[perfectionist]` table of `dylint.toml` and stash the
/// resolved override map. Called from
/// [`crate::register_lints`] immediately after
/// [`dylint_linting::init_config`] so that every per-rule
/// `register_pass` can consult [`is_enabled`] when deciding whether
/// to install its pass.
///
/// Panics if any rule name appears under both `enable` and
/// `disable` — that's a contradiction the runtime can't sensibly
/// resolve, and silently picking one direction would hide a
/// user-side mistake. Unknown rule names are silently ignored: the
/// override map keys that don't match any registered rule have no
/// effect (the rule never registers, so there's nothing to toggle),
/// and validating against the registered set here would duplicate
/// the existing `perfectionist::unknown_perfectionist_lints` rule's
/// purpose at a config-loading layer that has no diagnostic surface.
pub(crate) fn init_global_config() {
    let config: GlobalConfig = dylint_linting::config_or_default("perfectionist");
    let mut overrides: HashMap<String, bool> = HashMap::new();
    for (selectors, enabled) in [(&config.enable, true), (&config.disable, false)] {
        for selector in selectors {
            let name = selector.name();
            if let Some(prev) = overrides.insert(name.to_owned(), enabled)
                && prev != enabled
            {
                panic!(
                    "perfectionist: rule `{name}` listed under both `enable` and \
                     `disable` in the `[perfectionist]` table of `dylint.toml`",
                );
            }
        }
    }
    GLOBAL_OVERRIDES
        .set(overrides)
        .expect("init_global_config called twice");
}

/// Whether the rule named `name` (unqualified — no `perfectionist::`
/// prefix) should have its pass installed. Resolution order:
///
/// 1. If `name` appears under `disable` in the `[perfectionist]`
///    table, return `false`.
/// 2. If it appears under `enable`, return `true`.
/// 3. Otherwise return `default_enabled` — the per-rule baseline.
///
/// Each rule's `register_pass` passes its own baseline as
/// `default_enabled`: most rules pass `true`; rules listed in
/// `src/rules/<name>.rs` as `ENABLED_BY_DEFAULT: bool = false`
/// pass `false` and ship turned off until the user opts in.
pub(crate) fn is_enabled(name: &str, default_enabled: bool) -> bool {
    let overrides = GLOBAL_OVERRIDES
        .get()
        .expect("is_enabled called before init_global_config");
    overrides.get(name).copied().unwrap_or(default_enabled)
}

/// Whether `name` is exactly one ASCII letter (`a`..=`z` or
/// `A`..=`Z`). Used by every `single_letter_*` rule.
pub(crate) fn is_single_ascii_letter(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    chars.next().is_none() && first.is_ascii_alphabetic()
}

/// Extract the identifier from a plain `Binding(_, _, ident, None)`
/// pattern. Returns `None` for any non-binding pattern or a binding
/// with a sub-pattern. Used by the `let`-binding, function-parameter,
/// and closure-parameter rules.
pub(crate) fn binding_ident<'hir>(pat: &'hir hir::Pat<'hir>) -> Option<rustc_span::Ident> {
    match pat.kind {
        hir::PatKind::Binding(_, _, ident, None) => Some(ident),
        _ => None,
    }
}

/// Sibling of [`binding_ident`] that returns the binding's `HirId`
/// instead of its `Ident`. Used by the closure-parameter rule to test
/// whether a particular expression refers to one of the closure's
/// parameters.
pub(crate) fn binding_hir_id<'hir>(pat: &'hir hir::Pat<'hir>) -> Option<hir::HirId> {
    match pat.kind {
        hir::PatKind::Binding(_, hir_id, _, None) => Some(hir_id),
        _ => None,
    }
}

/// Merge a curated built-in allowlist of `&str` defaults with a
/// user-supplied `extras` list, then subtract every entry in
/// `ignore`. Used by rules whose runtime allowlist key remains
/// a `String` (currently just `non_exhaustive_error`, whose
/// suffix lookup is `str::ends_with`-shaped); the four rules
/// whose late-pass lookup key is a [`Symbol`] use the sibling
/// [`merge_symbol_allowlist`] instead. The `BTreeSet` return is
/// convenient for set membership lookups and has the side
/// benefit of dropping duplicates when defaults and extras
/// overlap; callers that need a `Vec`-shaped result can
/// `.into_iter().collect()` it themselves.
pub(crate) fn merge_string_allowlist(
    defaults: &[&str],
    extras: Vec<String>,
    ignore: Vec<String>,
) -> BTreeSet<String> {
    let ignore: BTreeSet<String> = ignore.into_iter().collect();
    defaults
        .iter()
        .map(ToString::to_string)
        .chain(extras)
        .filter(|name| !ignore.contains(name))
        .collect()
}

/// Sibling of [`merge_string_allowlist`] that interns each name as
/// a [`Symbol`] in one pass — skipping the intermediate
/// `BTreeSet<String>` of the string-shaped variant. Used by rules
/// whose late-pass lookup key is already a `Symbol`
/// (`unicode_ellipsis_in_panic_messages`, the three `single_letter_*`
/// rules), so that membership checks reduce to integer compares
/// instead of `Symbol::as_str` → `String` round-trips.
///
/// Must be called inside a rustc session, since [`Symbol::intern`]
/// reaches into the per-session symbol table.
pub(crate) fn merge_symbol_allowlist(
    defaults: &[&str],
    extras: Vec<String>,
    ignore: Vec<String>,
) -> BTreeSet<Symbol> {
    let ignore: BTreeSet<Symbol> = ignore.iter().map(|name| Symbol::intern(name)).collect();
    defaults
        .iter()
        .map(|name| Symbol::intern(name))
        .chain(extras.iter().map(|name| Symbol::intern(name)))
        .filter(|sym| !ignore.contains(sym))
        .collect()
}
