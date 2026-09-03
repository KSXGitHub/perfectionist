use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::{DefaultState, resolve_symbol_set, resolved_state};
use crate::enclosing_hir::emit_at_enclosing_hir;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use core::num::NonZeroUsize;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_hir::def::{DefKind, Namespace, Res};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::def_id::{CRATE_DEF_ID, CrateNum, DefId, LocalDefId};
use rustc_span::{Span, Symbol};
use std::collections::BTreeSet;

mod casing;
mod scan;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a backticked identifier in a doc comment (`` `Foo` ``)
    /// that resolves as a Rust path in the documented item's scope but
    /// is not written as a rustdoc intra-doc link (`` [`Foo`] ``).
    ///
    /// Only bare single identifiers whose name resolves to an item in
    /// the enclosing module's scope are flagged; a backticked word that
    /// names nothing in scope is left alone.
    ///
    /// A publicly-reachable item that mentions a *private* (not
    /// publicly-reachable) item is also left alone: turning that mention
    /// into a link would make rustdoc's `rustdoc::private_intra_doc_links`
    /// fire under a plain `cargo doc`, and a public item leaning on a
    /// private one is a separate concern from this rule's.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Both
    /// `` `Foo` `` and `` [`Foo`] `` render as monospaced text, so the
    /// page looks the same at a glance. The link form additionally
    /// turns the mention into a clickable cross-reference and lets
    /// rustdoc's `rustdoc::broken_intra_doc_links` lint catch the day a
    /// rename leaves the prose pointing at a type that no longer
    /// exists. Spelling every in-scope mention as a link keeps the
    /// documentation navigable and the references checked.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// /// Installs the package described by `PackageManifest` into `Store`.
    /// pub fn install(manifest: &PackageManifest, store: &Store) {}
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// /// Installs the package described by [`PackageManifest`] into [`Store`].
    /// pub fn install(manifest: &PackageManifest, store: &Store) {}
    /// ```
    ///
    /// ### Caveat: linking is not always correct
    ///
    /// When a backticked word names a *foreign* symbol that merely
    /// shares its name with an in-scope item, rewriting `` `Foo` `` to
    /// `` [`Foo`] `` links to the *local* item — a wrong reference that
    /// still compiles. The rule therefore only suggests the link
    /// rather than applying it automatically. For an external target,
    /// write an explicit reference link instead:
    /// `` [`Foo`][foo-ext] `` plus a `` [foo-ext]: <url> `` definition.
    pub perfectionist::BARE_IDENTIFIER_REFERENCE,
    Warn,
    "backticked identifier in a doc comment that resolves in scope should be an intra-doc link",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::bare_identifier_reference";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Identifiers the rule never suggests linking, even when they
    /// resolve in scope. Empty by default. Use this for a name a doc
    /// comment deliberately mentions without wanting a cross-reference
    /// — a historical type kept for context, or a word that happens to
    /// collide with an in-scope item but is meant as prose.
    skip_idents: Vec<String>,
    /// How far from the documenting item a referenced name may resolve
    /// for the rule to check it, by where the referenced item lives in
    /// the module tree. A backticked word that matches an
    /// accidentally-added cross-module import is a common source of
    /// churn, so a project can narrow (or widen) this. Defaults to
    /// `crate`: a project's own items are kept linked, but mentions of
    /// the standard library and third-party crates are left alone.
    reference_scope: ReferenceScope,
    /// Whether to check `PascalCase` names. Defaults to `true`.
    ///
    /// A mixed or non-conformist name (`fooBar`, `foo_BAR`, `__foo`,
    /// `foo__bar`) is checked regardless of this field: such a spelling
    /// is rare and, when it matches a local identifier, rarely an
    /// accident.
    check_pascal_case: bool,
    /// Whether to check `UPPER_CASE` (`SCREAMING_SNAKE_CASE`) names.
    /// Defaults to `true`.
    ///
    /// A mixed or non-conformist name (`fooBar`, `foo_BAR`, `__foo`,
    /// `foo__bar`) is checked regardless of this field: such a spelling
    /// is rare and, when it matches a local identifier, rarely an
    /// accident.
    check_upper_case: bool,
    /// Whether to check `snake_case` names. Defaults to `true`.
    ///
    /// A mixed or non-conformist name (`fooBar`, `foo_BAR`, `__foo`,
    /// `foo__bar`) is checked regardless of this field: such a spelling
    /// is rare and, when it matches a local identifier, rarely an
    /// accident.
    check_snake_case: bool,
    /// Minimum number of words a name must have to be checked. Defaults
    /// to `1` (check everything). At `3`, `foo`, `foo_bar`, `Foo`,
    /// `FooBar`, `FOO`, and `FOO_BAR` are exempt, while `foo_bar_baz`,
    /// `FooBarBaz`, and `FOO_BAR_BAZ` are checked.
    ///
    /// A mixed or non-conformist name (`fooBar`, `foo_BAR`, `__foo`,
    /// `foo__bar`) is checked regardless of this threshold: such a
    /// spelling is rare and, when it matches a local identifier, rarely
    /// an accident.
    min_words: NonZeroUsize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            skip_idents: Vec::new(),
            reference_scope: ReferenceScope::default(),
            check_pascal_case: true,
            check_upper_case: true,
            check_snake_case: true,
            min_words: NonZeroUsize::MIN,
        }
    }
}

/// How far from the documenting item a referenced name may resolve for
/// the rule to check it, configured by `reference_scope`. The axis is
/// *where the referenced item lives relative to the documenting item's
/// module*, not the spelling of the `use` path that brought it in.
#[derive(Debug, Default, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReferenceScope {
    /// Only items defined directly in the documenting item's own module.
    /// Every name reached through a `use` is left alone.
    OwnModule,
    /// Also items from within that module's own subtree (e.g. reached
    /// through `use self::child::Item`), but still not names that reach
    /// outside the module — `use super::...`, `use crate::...`, and
    /// imports from other crates.
    ModuleTree,
    /// Any item defined anywhere in the current crate (first-party), but
    /// nothing from another crate. A project's own items are where
    /// documentation drifts — a rename leaves a stale mention — while the
    /// standard library and third-party crates are stable and outside the
    /// project's control, so a bare mention of them is low risk.
    #[default]
    Crate,
    /// The current crate and its third-party dependencies, but not the
    /// standard / built-in libraries (`std`, `core`, `alloc`,
    /// `proc_macro`, `test`). Use this when dependency references are
    /// worth checking but the frozen standard library is not.
    ThirdParty,
    /// Any name that resolves in scope, however it got there — including
    /// the standard library.
    Anywhere,
}

pub struct BareIdentifierReference {
    skip_idents: BTreeSet<Symbol>,
    reference_scope: ReferenceScope,
    check_pascal_case: bool,
    check_upper_case: bool,
    check_snake_case: bool,
    min_words: NonZeroUsize,
}

impl BareIdentifierReference {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let skip_idents = resolve_symbol_set(&[], config.skip_idents, Vec::new());
        Self {
            skip_idents,
            reference_scope: config.reference_scope,
            check_pascal_case: config.check_pascal_case,
            check_upper_case: config.check_upper_case,
            check_snake_case: config.check_snake_case,
            min_words: config.min_words,
        }
    }

    /// Whether the candidate's name passes the case and minimum-word-count
    /// knobs. A mixed / non-conformist name is always checked — those
    /// knobs only gate the three conformist shapes — so it short-circuits
    /// to `true`.
    fn name_allows(&self, ident: &str) -> bool {
        let case = casing::classify(ident);
        let case_enabled = match case {
            casing::Case::Snake => self.check_snake_case,
            casing::Case::Upper => self.check_upper_case,
            casing::Case::Pascal => self.check_pascal_case,
            casing::Case::NonConformist => return true,
        };
        if !case_enabled {
            return false;
        }
        // The default `min_words == 1` admits every name, so skip the
        // word count (`pascal_word_count` allocates) on the hot path.
        self.min_words == NonZeroUsize::MIN
            || casing::word_count(ident, case) >= self.min_words.get()
    }
}

impl_lint_pass!(BareIdentifierReference => [BARE_IDENTIFIER_REFERENCE]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[BARE_IDENTIFIER_REFERENCE]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive =
        resolved_state("bare_identifier_reference", DefaultState::Active)
    {
        return;
    }
    lint_store.register_late_lint_pass(Box::new(|_| Box::new(BareIdentifierReference::new())));
}

/// One parked finding: the identifier text plus the source snippet of
/// the whole `` `Foo` `` code span, used to build the autofix.
struct Violation {
    ident: Symbol,
    snippet: String,
}

impl<'tcx> LateLintPass<'tcx> for BareIdentifierReference {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let mut violations: Vec<(Span, Violation)> = Vec::new();
        walk_local_comments(lint_context, |chunk| match chunk.surface {
            CommentSurface::DocBlock | CommentSurface::DocBlockBlock => {
                self.collect_doc_chunk(lint_context, chunk, &mut violations);
            }
            CommentSurface::PlainLine | CommentSurface::PlainBlock => {}
        });
        emit_at_enclosing_hir(lint_context.tcx, violations, |hir_id, span, violation| {
            // Resolution is deferred to here: `emit_at_enclosing_hir`
            // has just told us which HIR node the doc comment documents,
            // which is the scope a rustdoc intra-doc link resolves in.
            if let Some(resolution) = self.resolve_in_scope(lint_context, hir_id, violation.ident) {
                emit(lint_context, hir_id, span, &violation, resolution);
            }
        });
    }
}

impl BareIdentifierReference {
    fn collect_doc_chunk(
        &self,
        lint_context: &LateContext<'_>,
        chunk: &CommentChunk<'_>,
        out: &mut Vec<(Span, Violation)>,
    ) {
        for candidate in scan::collect_candidates(&chunk.rendered) {
            // Cheapest-first: the pure-`&str` case / word-count filter
            // rejects most prose words before interning a `Symbol`.
            if !self.name_allows(&candidate.ident) {
                continue;
            }
            let name = Symbol::intern(&candidate.ident);
            if self.skip_idents.contains(&name) {
                continue;
            }
            let len = (candidate.span.end - candidate.span.start) as u32;
            let Some(span) = chunk.span_for(candidate.span.start, len) else {
                continue;
            };
            // Prefer the real source text for the autofix so the
            // backtick run and any padding are preserved verbatim; fall
            // back to the rendered slice if the span isn't snippet-able.
            let snippet = lint_context
                .sess()
                .source_map()
                .span_to_snippet(span)
                .unwrap_or_else(|_| chunk.rendered[candidate.span.clone()].to_owned());
            out.push((
                span,
                Violation {
                    ident: name,
                    snippet,
                },
            ));
        }
    }
}

/// How a candidate identifier resolves in the documented item's scope.
#[derive(Clone, Copy)]
enum Resolution {
    /// Resolves in exactly one namespace — a plain `` [`Foo`] `` link
    /// is unambiguous, so the rule offers it as a structured
    /// suggestion (only `MaybeIncorrect`: see [`emit`], as the name may
    /// instead denote a foreign symbol that merely shares it).
    Unique,
    /// The name exists in more than one namespace (e.g. a type and a
    /// function). A bare `` [`Foo`] `` would be an ambiguous intra-doc
    /// link, so the rule emits a help note rather than an autofix,
    /// carrying the disambiguator prefix to suggest.
    Ambiguous(DisambiguatorPrefix),
}

/// The rustdoc disambiguator prefix the ambiguity help suggests. Chosen
/// from a namespace that has an *eligible* target, so the suggested link
/// never points the reader at a private / out-of-policy item.
#[derive(Clone, Copy)]
enum DisambiguatorPrefix {
    Type,
    Value,
    Macro,
}

impl DisambiguatorPrefix {
    fn as_str(self) -> &'static str {
        match self {
            DisambiguatorPrefix::Type => "type@",
            DisambiguatorPrefix::Value => "value@",
            DisambiguatorPrefix::Macro => "macro@",
        }
    }

    /// Pick the prefix for the first present namespace, in rustdoc's
    /// type → value → macro precedence. `mask` is `[type, value, macro]`.
    fn from_mask(mask: [bool; 3]) -> Option<Self> {
        if mask[0] {
            Some(DisambiguatorPrefix::Type)
        } else if mask[1] {
            Some(DisambiguatorPrefix::Value)
        } else if mask[2] {
            Some(DisambiguatorPrefix::Macro)
        } else {
            None
        }
    }
}

impl BareIdentifierReference {
    /// Resolve `name` against the children of the documented item's
    /// scope module. Returns `None` when the name names nothing in scope
    /// the rule still checks (so the backticks are deliberate prose, not
    /// an unlinked reference).
    ///
    /// Ambiguity and eligibility are judged on different sets, because
    /// they answer different questions:
    ///
    /// - **Ambiguity** (does a bare `` [`Foo`] `` resolve uniquely?) is
    ///   judged the way rustdoc resolves the link: across *every*
    ///   same-name item in scope, in every namespace, regardless of
    ///   privacy or the project's `reference_scope` — those are the
    ///   rule's filters, not rustdoc's. A unit/tuple-struct constructor
    ///   shares its type's identity and so never counts.
    /// - **Eligibility** (is the rule willing to point a link at this?)
    ///   applies the rule's own filters: the `reference_scope` policy
    ///   (see [`ReferenceScope`]) drops a target that lives farther out
    ///   than the project allows, and the public-references-private
    ///   exemption drops a private target a `pub` item shouldn't link to
    ///   (turning `` `Priv` `` into `` [`Priv`] `` would make rustdoc's
    ///   `rustdoc::private_intra_doc_links` fire under a plain
    ///   `cargo doc`; that is a separate rule's concern).
    ///
    /// Returns `None` when no in-scope child is eligible (the backticks
    /// are deliberate prose, not an unlinked reference).
    fn resolve_in_scope(
        &self,
        cx: &LateContext<'_>,
        hir_id: hir::HirId,
        name: Symbol,
    ) -> Option<Resolution> {
        let scope = scope_module(cx, hir_id);
        let effective_visibilities = cx.tcx.effective_visibilities(());
        let documented_public = effective_visibilities.is_reachable(documented_def_id(cx, hir_id));
        // One slot per namespace (`TypeNS`, `ValueNS`, `MacroNS`). The
        // first set is rustdoc's resolution view (for ambiguity); the
        // second is the subset that survives the rule's filters (for
        // choosing a disambiguator that points at an endorsed target).
        let mut namespaces = [false; 3];
        let mut eligible_namespaces = [false; 3];
        let mut has_eligible_target = false;
        for child in cx.tcx.module_children_local(scope) {
            if child.ident.name != name {
                continue;
            }
            let target = child.res.opt_def_id();
            // A unit/tuple-struct constructor shares its type's identity,
            // so it never counts toward ambiguity; map everything else to
            // its namespace slot.
            let ns_slot = if matches!(child.res, Res::Def(DefKind::Ctor(..), _)) {
                None
            } else {
                match child.res.ns() {
                    Some(Namespace::TypeNS) => Some(0),
                    Some(Namespace::ValueNS) => Some(1),
                    Some(Namespace::MacroNS) => Some(2),
                    None => None,
                }
            };

            // Ambiguity accounting (rustdoc's view): record every
            // same-name child's namespace, ignoring privacy and policy.
            if let Some(slot) = ns_slot {
                namespaces[slot] = true;
            }

            // Eligibility (the rule's filters): `reference_scope` drops a
            // target living farther out than the project allows; the
            // public-references-private exemption drops a private target
            // (a local item that is not publicly reachable — an external
            // target such as a re-exported `std` type is public).
            let reach = target.map_or(Reach::ThirdPartyCrate, |def_id| {
                import_reach(cx, scope, def_id)
            });
            let ignored_by_policy = match self.reference_scope {
                ReferenceScope::OwnModule => reach != Reach::LocalDefinition,
                ReferenceScope::ModuleTree => {
                    !matches!(reach, Reach::LocalDefinition | Reach::InternalImport)
                }
                ReferenceScope::Crate => !reach.is_in_current_crate(),
                ReferenceScope::ThirdParty => reach == Reach::StandardLibrary,
                ReferenceScope::Anywhere => false,
            };
            if ignored_by_policy {
                continue;
            }
            let target_private = target
                .and_then(|def_id| def_id.as_local())
                .is_some_and(|local| !effective_visibilities.is_reachable(local));
            if documented_public && target_private {
                continue;
            }
            has_eligible_target = true;
            if let Some(slot) = ns_slot {
                eligible_namespaces[slot] = true;
            }
        }
        if !has_eligible_target {
            return None;
        }
        let distinct = namespaces.iter().filter(|present| **present).count();
        Some(if distinct > 1 {
            // Prefer a namespace that has an eligible target so the help
            // never steers the reader at a private / out-of-policy item;
            // fall back to the full set if the only eligible target
            // carries no namespace (e.g. a lone ctor). `distinct > 1`
            // guarantees the chosen mask has a present namespace.
            let prefix_set = if eligible_namespaces.iter().any(|present| *present) {
                eligible_namespaces
            } else {
                namespaces
            };
            let prefix = DisambiguatorPrefix::from_mask(prefix_set)
                .expect("an ambiguous resolution always has a present namespace");
            Resolution::Ambiguous(prefix)
        } else {
            Resolution::Unique
        })
    }
}

/// Where a resolved candidate's target lives relative to the documented
/// item's module and crate — the axis [`ReferenceScope`] filters on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// Defined directly in the documenting item's own module (not an
    /// import at all).
    LocalDefinition,
    /// Defined within that module's own subtree, reached through a
    /// `use self::...` of a descendant module's item.
    InternalImport,
    /// Defined elsewhere in the current crate — a `use super::...` /
    /// `use crate::...` of another module.
    CrateElsewhere,
    /// Defined in a third-party dependency (an external crate that is not
    /// one of the standard / built-in libraries).
    ThirdPartyCrate,
    /// Defined in a standard / built-in library: `std`, `core`, `alloc`,
    /// `proc_macro`, or `test`.
    StandardLibrary,
}

impl Reach {
    /// Whether the target lives in the current crate (first-party).
    fn is_in_current_crate(self) -> bool {
        matches!(
            self,
            Reach::LocalDefinition | Reach::InternalImport | Reach::CrateElsewhere,
        )
    }
}

/// Classify where `target` lives relative to the `scope` module and the
/// current crate, by the defining module's position in the module tree
/// (not the `use` path's spelling).
fn import_reach(cx: &LateContext<'_>, scope: LocalDefId, target: DefId) -> Reach {
    if !target.is_local() {
        return if is_standard_library(cx, target.krate) {
            Reach::StandardLibrary
        } else {
            Reach::ThirdPartyCrate
        };
    }
    let scope_def = scope.to_def_id();
    let Some(defining_module) = cx.tcx.opt_parent(target) else {
        return Reach::CrateElsewhere;
    };
    if defining_module == scope_def {
        return Reach::LocalDefinition;
    }
    // Walk the defining module's ancestry; reaching the scope module
    // means the target sits in the scope's own subtree.
    let mut module = Some(defining_module);
    while let Some(current) = module {
        if current == scope_def {
            return Reach::InternalImport;
        }
        module = cx.tcx.opt_parent(current);
    }
    Reach::CrateElsewhere
}

/// Whether `krate` is a standard / built-in library that ships with the
/// compiler — stable enough that doc references into it are low-risk, so
/// the narrower [`ReferenceScope`] levels exclude it. Keyed on the
/// *defining* crate's name, so a `std` re-export of a `core` item
/// (`Option`) is recognised through its `core` origin.
fn is_standard_library(cx: &LateContext<'_>, krate: CrateNum) -> bool {
    matches!(
        cx.tcx.crate_name(krate).as_str(),
        "core" | "alloc" | "std" | "proc_macro" | "test",
    )
}

/// The [`LocalDefId`] of the item a doc comment documents, for the
/// public-vs-private visibility check. Fields and enum variants carry
/// their own `def_id`; every other anchor (items, trait / impl /
/// foreign items, and the crate root) is an HIR owner, so its
/// `hir_id.owner` is the right def id.
fn documented_def_id(cx: &LateContext<'_>, hir_id: hir::HirId) -> LocalDefId {
    match cx.tcx.hir_node(hir_id) {
        hir::Node::Field(field) => field.def_id,
        hir::Node::Variant(variant) => variant.def_id,
        _ => hir_id.owner.def_id,
    }
}

/// The module whose scope a rustdoc intra-doc link on the node at
/// `hir_id` resolves in. For a module (or the crate root) that is the
/// node itself — its own `//!` / `///` doc resolves against its
/// contents; for any other item it is the enclosing module.
fn scope_module(cx: &LateContext<'_>, hir_id: hir::HirId) -> LocalDefId {
    match cx.tcx.hir_node(hir_id) {
        hir::Node::Crate(_) => CRATE_DEF_ID,
        hir::Node::Item(item) if matches!(item.kind, hir::ItemKind::Mod(..)) => {
            item.owner_id.def_id
        }
        _ => cx.tcx.parent_module(hir_id).to_local_def_id(),
    }
}

fn emit(
    cx: &LateContext<'_>,
    hir_id: hir::HirId,
    span: Span,
    violation: &Violation,
    resolution: Resolution,
) {
    let Violation { ident, snippet } = violation;
    span_lint_hir_and_then(
        cx,
        BARE_IDENTIFIER_REFERENCE,
        hir_id,
        span,
        format!("`{ident}` resolves in scope; consider writing it as an intra-doc link"),
        |diag| {
            match resolution {
                Resolution::Unique => {
                    // `MaybeIncorrect`, not `MachineApplicable`: the rewrite
                    // changes meaning when the prose names a foreign symbol
                    // that merely shares this name (see the caveat help
                    // below), so `cargo fix` / `cargo clippy --fix` and an
                    // editor's "apply suggestion" must not apply it blindly.
                    diag.span_suggestion(
                        span,
                        "only once the surrounding context confirms it refers to this \
                         item, not a foreign symbol of the same name, wrap it as an \
                         intra-doc link",
                        format!("[{snippet}]"),
                        Applicability::MaybeIncorrect,
                    );
                }
                Resolution::Ambiguous(prefix) => {
                    let prefix = prefix.as_str();
                    diag.help(format!(
                        "`{ident}` resolves in more than one namespace; only once the \
                         surrounding context confirms it refers to this item, not a \
                         foreign symbol of the same name, write a disambiguated \
                         intra-doc link such as `[`{ident}`]({prefix}{ident})`",
                    ));
                }
            }
            // A backticked name can denote a foreign/upstream symbol the doc
            // only mentions, coinciding with an in-scope item; an intra-doc
            // link would then resolve to the wrong target while still
            // compiling. Point authors at an explicit reference link to the
            // real source for that case.
            diag.help(format!(
                "if `{ident}` instead names a foreign or upstream symbol that the doc \
                 only mentions by name, do not link it to the local item; write an \
                 explicit reference link to the real source, such as \
                 `[`{ident}`][{ident}-ext]` with a `[{ident}-ext]: <url>` definition",
            ));
        },
    );
}
