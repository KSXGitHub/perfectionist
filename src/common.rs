//! Helpers shared between sibling rules.
//!
//! Each helper lives here only because more than one rule needs it.
//! Anything used by a single rule belongs in that rule's own file.

use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

use rustc_ast::{MetaItemInner, MetaItemKind, MetaItemLit};
use rustc_hir as hir;
use rustc_hir::HirId;
use rustc_lint::{LateContext, LintContext};
use rustc_span::{Span, Symbol, sym};

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
/// single literal array. The table shape is its own struct so we
/// can put `#[serde(deny_unknown_fields)]` on it — serde doesn't
/// honour that attribute on inline enum-variant shapes, only on
/// named structs.
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum RuleSelector {
    Name(String),
    Verbose(VerboseSelector),
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerboseSelector {
    name: String,
    #[expect(
        dead_code,
        reason = "decorative field for human readers of dylint.toml"
    )]
    reason: Option<String>,
}

impl RuleSelector {
    fn name(&self) -> &str {
        match self {
            RuleSelector::Name(name) => name,
            RuleSelector::Verbose(verbose) => &verbose.name,
        }
    }
}

/// Whether a rule's pass is installed at all. Both the per-rule
/// baseline (each rule's `DEFAULT_STATE` constant) and the
/// user-supplied override (the `enable` / `disable` arrays in
/// `dylint.toml`) speak this same alphabet, so a single type is
/// used end-to-end and no `bool` ever bridges the two. Mirrors
/// `gen-docs`'s own `DefaultState` (separate crate, same shape) —
/// the doc generator reads each rule's `DEFAULT_STATE` constant
/// directly to render the rule's catalogue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefaultState {
    Active,
    Inactive,
}

/// Resolved per-rule override map, built once from the
/// `[perfectionist]` table of `dylint.toml`. Rules absent from this
/// map fall through to the per-rule `DEFAULT_STATE` constant each
/// `register_pass` passes to [`resolved_state`].
static GLOBAL_OVERRIDES: OnceLock<HashMap<String, DefaultState>> = OnceLock::new();

/// Parse the `[perfectionist]` table of `dylint.toml` and stash the
/// resolved override map. Called from
/// [`crate::register_lints`] immediately after
/// [`dylint_linting::init_config`] so that every per-rule
/// `register_pass` can consult [`resolved_state`] when deciding
/// whether to install its pass.
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
    let mut overrides: HashMap<String, DefaultState> = HashMap::new();
    for (selectors, state) in [
        (&config.enable, DefaultState::Active),
        (&config.disable, DefaultState::Inactive),
    ] {
        for selector in selectors {
            let name = selector.name();
            if let Some(previous) = overrides.insert(name.to_owned(), state)
                && previous != state
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

/// Effective [`DefaultState`] for the rule named `name` (unqualified
/// — no `perfectionist::` prefix). Resolution order:
///
/// 1. If `name` appears under `disable` in the `[perfectionist]`
///    table, return [`DefaultState::Inactive`].
/// 2. If it appears under `enable`, return [`DefaultState::Active`].
/// 3. Otherwise return `default` — the per-rule baseline.
///
/// Each rule's `register_pass` passes its own baseline as
/// `default`: most rules pass [`DefaultState::Active`]; rules
/// listed in `src/rules/<name>.rs` as
/// `DEFAULT_STATE: DefaultState = DefaultState::Inactive` pass
/// `Inactive` and ship turned off until the user opts in.
pub(crate) fn resolved_state(name: &str, default: DefaultState) -> DefaultState {
    let overrides = GLOBAL_OVERRIDES
        .get()
        .expect("resolved_state called before init_global_config");
    overrides.get(name).copied().unwrap_or(default)
}

/// Look up the `reason = "<literal>"` field in a lint-level attribute's
/// argument list. Returns the `MetaItemLit` (so callers can inspect the
/// literal's text and span) or `None` if no `reason` field is present.
///
/// Shared between the lint-level rules that all consume the same
/// notion of "this attribute carries an explanatory reason":
/// `lint_silence_reason`, `lint_downgrade_reason`, and
/// `lint_reason_from_comment`. The arg list is the post-`meta_item_list`
/// vector of nested meta items — the same shape every caller already
/// constructs from `Attribute::meta_item_list()`.
pub(crate) fn attr_has_reason(args: &[MetaItemInner]) -> Option<&MetaItemLit> {
    for arg in args {
        let MetaItemInner::MetaItem(meta) = arg else {
            continue;
        };
        if !meta.has_name(sym::reason) {
            continue;
        }
        let MetaItemKind::NameValue(literal) = &meta.kind else {
            continue;
        };
        return Some(literal);
    }
    None
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

/// Resolve a `&str` set from a curated built-in default, a
/// user-supplied `extras` list, and a user-supplied `ignore`
/// list. Used by rules whose runtime set key remains a `String`
/// (currently just `non_exhaustive_error`, whose suffix lookup is
/// `str::ends_with`-shaped); the four rules whose late-pass
/// lookup key is a [`Symbol`] use the sibling
/// [`resolve_symbol_set`] instead. The `BTreeSet` return is
/// convenient for set membership lookups and has the side
/// benefit of dropping duplicates when defaults and extras
/// overlap; callers that need a `Vec`-shaped result can
/// `.into_iter().collect()` it themselves.
pub(crate) fn resolve_string_set(
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

/// Sibling of [`resolve_string_set`] that interns each name as
/// a [`Symbol`] in one pass — skipping the intermediate
/// `BTreeSet<String>` of the string-shaped variant. Used by rules
/// whose late-pass lookup key is already a `Symbol`
/// (`unicode_ellipsis_in_panic_messages`, `single_letter_closure_param`'s
/// trivial-callback list), so that membership checks reduce to
/// integer compares instead of `Symbol::as_str` → `String`
/// round-trips.
///
/// Must be called inside a rustc session, since [`Symbol::intern`]
/// reaches into the per-session symbol table.
pub(crate) fn resolve_symbol_set(
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

/// Sibling of [`resolve_symbol_set`] keyed on [`char`] instead of
/// `String`, for the `single_letter_*` rules' `extra_allowed_idents`
/// and `ignore_allowed_idents` knobs. `char::encode_utf8` writes
/// into a small stack buffer and hands [`Symbol::intern`] the
/// resulting `&str`. `extras` and `ignore` accept anything that
/// iterates into `char` — notably the rules'
/// `Vec<crate::ascii_letter::AsciiLetter>` knobs, whose `Into<char>`
/// keeps the ASCII-letter restriction in the type rather than at
/// this boundary — so each caller hands its config field straight
/// in without a per-call conversion.
///
/// Must be called inside a rustc session.
pub(crate) fn resolve_symbol_set_from_chars(
    defaults: &[char],
    extras: impl IntoIterator<Item: Into<char>>,
    ignore: impl IntoIterator<Item: Into<char>>,
) -> BTreeSet<Symbol> {
    let intern = |ch: char| {
        let mut buf = [0u8; 4];
        Symbol::intern(ch.encode_utf8(&mut buf))
    };
    let ignore: BTreeSet<Symbol> = ignore.into_iter().map(Into::into).map(intern).collect();
    defaults
        .iter()
        .copied()
        .chain(extras.into_iter().map(Into::into))
        .map(intern)
        .filter(|sym| !ignore.contains(sym))
        .collect()
}
