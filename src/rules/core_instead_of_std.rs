//! `perfectionist::core_instead_of_std` — flag an item named through
//! `core::` or `alloc::` in a crate that has settled on `std`, and
//! suggest the `std::` spelling.
//!
//! The mirror image of `clippy::std_instead_of_core` and
//! `clippy::std_instead_of_alloc`, which push a path towards the
//! narrower crate so a library stays portable to `no_std`. Clippy ships
//! no lint in this direction, which is the whole reason this rule
//! exists. Two shapes of the problem drive the implementation, and both
//! were learned from the clippy lints that face the other way.
//!
//! **One token, several paths.** HIR lowers
//! `use core::{fmt::Display, ops::Add};` into one `Use` item per leaf,
//! and `walk_use` visits each leaf once per namespace its name resolves
//! in — every visit carrying the *same* `core` token span. The fix
//! rewrites that one token, so the paths written through it have to be
//! judged together: a leaf that must keep its `core` spelling vetoes
//! the rewrite for its siblings. [`emit`] buffers the paths sharing a
//! token into a group and decides once.
//!
//! **The suffix has to name the same item.** Swapping only the crate
//! segment is not sound on its own. `core::panic::PanicInfo` and
//! `std::panic::PanicInfo` are different types — std's is a deprecated
//! alias of `PanicHookInfo` — so the rewritten path is re-resolved and
//! accepted only when it lands on the very same `DefId`.

use crate::rule_index::{Register, rule};
use clippy_utils::paths::{PathNS, lookup_path};
use rustc_hir::def::{DefKind, Namespace, Res};
use rustc_hir::{Block, Body, HirId, Path, PathSegment, find_attr};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::def_id::DefId;
use rustc_span::{Span, Symbol, kw, sym};

mod config;
mod emit;

use crate::common::{DefaultState, hir_in_external_macro, join_path_segments};
use config::{Config, Resolved};
use emit::{Group, Point};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags an item named through `core::` or `alloc::` when the same
    /// item is reachable through `std::`, and rewrites the leading crate
    /// segment to `std`. `alloc::` paths are covered too unless
    /// `also_alloc` turns that half off, and individual paths can be
    /// exempted with `skip_paths`.
    ///
    /// This is the counterpart of `clippy::std_instead_of_core` and
    /// `clippy::std_instead_of_alloc`, which push paths the other way —
    /// towards the narrower crate — so that a library keeps compiling
    /// without `std`. Enable those two on a crate that is, or may become,
    /// `no_std`; enable this one on a crate that has settled on `std`.
    /// The two directions contradict each other, so no crate wants both.
    ///
    /// Some paths are deliberately left alone. Nothing is flagged in a
    /// `#![no_std]` crate, where there is no `std::` to name. A path is
    /// left alone when its `std::` spelling would name a *different*
    /// item — `core::panic::PanicInfo` is not `std::panic::PanicInfo` —
    /// and so is a macro path, because `core::panic!` and `std::panic!`
    /// are genuinely different macros. A path produced by a macro
    /// expansion is left alone as well.
    ///
    /// Where one `core` token is shared by several names
    /// (`use core::{fmt::Display, panic::PanicInfo};`), the rewrite is
    /// offered only when every one of them is reachable through `std::`;
    /// otherwise the names that are get a `help` instead, since
    /// rewriting the shared token would change the others too.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue.
    /// Everything public in `core` and `alloc` is reachable through
    /// `std`, so in a crate that will always link `std` the spellings are
    /// interchangeable, and which one a given line uses is usually an
    /// accident — whichever crate the code was pasted from, or whichever
    /// path an editor's auto-import happened to offer. Naming one crate
    /// root throughout keeps every `use` line starting with the same
    /// word, and keeps an `alloc::` path from requiring an
    /// `extern crate alloc;` that a `std` crate has no other use for.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// use core::fmt::Display;
    /// use alloc::sync::Arc;
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// use std::fmt::Display;
    /// use std::sync::Arc;
    /// ```
    pub perfectionist::CORE_INSTEAD_OF_STD,
    Warn,
    "item named through `core` or `alloc` instead of `std`",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::core_instead_of_std";

pub struct CoreInsteadOfStd {
    config: Resolved,
    /// Whether the crate under lint is `#![no_std]`, in which case the
    /// pass has nothing to say. Set once at [`LateLintPass::check_crate`].
    no_std: bool,
    /// The paths seen so far that share one `core` / `alloc` token, held
    /// until a path arrives from a different token (or the enclosing
    /// block, body, or crate ends) and the group can be judged whole.
    pending: Option<Group>,
}

impl_lint_pass!(CoreInsteadOfStd => [CORE_INSTEAD_OF_STD]);

impl Register for rule::CoreInsteadOfStd {
    /// Inactive by default. Naming every path through `std` is a
    /// project-level commitment, and a `no_std` library wants the exact
    /// opposite, so the rule ships with no baseline; enable it in
    /// `[perfectionist].enable`.
    const DEFAULT_STATE: DefaultState = DefaultState::Inactive;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[CORE_INSTEAD_OF_STD]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        // Only a `::core::` / `::alloc::` entry can ever match a flagged
        // path, so reject anything else loudly rather than letting a
        // typo'd exemption silently fail to exempt.
        config::validate(&config).unwrap_or_else(|message| {
            panic!("perfectionist::core_instead_of_std: {message}");
        });
        lint_store.register_late_lint_pass(Box::new(move |_| {
            Box::new(CoreInsteadOfStd {
                config: Resolved::from_config(config.clone()),
                no_std: false,
                pending: None,
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for CoreInsteadOfStd {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        self.no_std = find_attr!(cx.tcx, crate, NoStd);
    }

    fn check_path(&mut self, cx: &LateContext<'tcx>, path: &Path<'tcx>, hir_id: HirId) {
        if self.no_std {
            return;
        }
        let Some((crate_span, crate_name, point)) = self.classify(cx, path, hir_id) else {
            return;
        };
        match &mut self.pending {
            Some(group) if group.crate_span.overlaps(crate_span) => group.points.push(point),
            _ => {
                let finished = self.pending.replace(Group {
                    crate_span,
                    crate_name,
                    points: vec![point],
                });
                emit::flush(cx, finished);
            }
        }
    }

    // A group is normally closed by the next path written through a
    // different token; these three close the last one of each scope, so
    // nothing is left buffered at the end of a run.

    fn check_block_post(&mut self, cx: &LateContext<'tcx>, _: &'tcx Block<'tcx>) {
        emit::flush(cx, self.pending.take());
    }

    fn check_body_post(&mut self, cx: &LateContext<'tcx>, _: &Body<'tcx>) {
        emit::flush(cx, self.pending.take());
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        emit::flush(cx, self.pending.take());
    }
}

impl CoreInsteadOfStd {
    /// Judge one path: its crate token, the name that token spells, and
    /// whether that path can move to `std`. `None` for a path the rule
    /// has no opinion about at all — one not written through a covered
    /// crate — which must *not* join a group, since a path the rule
    /// ignores does not veto its siblings' rewrite.
    fn classify(
        &self,
        cx: &LateContext<'_>,
        path: &Path<'_>,
        hir_id: HirId,
    ) -> Option<(Span, Symbol, Point)> {
        let Res::Def(def_kind, def_id) = path.res else {
            return None;
        };
        // `core::panic!` and `std::panic!` expand differently, and a
        // macro is re-exported by a mechanism of its own; leave the
        // macro namespace to the human.
        if matches!(def_kind, DefKind::Macro(_)) {
            return None;
        }

        let first = first_segment(path)?;
        let last = path.segments.last()?;
        let crate_name = first.ident.name;
        if !self.covers(crate_name) {
            return None;
        }
        // The written `core` has to *be* the crate: a local `mod core`
        // shadowing it in some inner scope is not this rule's business.
        let Res::Def(DefKind::Mod, first_def_id) = first.res else {
            return None;
        };
        if !first_def_id.is_crate_root() {
            return None;
        }
        // The diagnostic span is one token of a longer path, so the
        // `report_in_external_macro: false` filter alone would let a
        // proc-macro-synthesised path through.
        if path.span.from_expansion() || hir_in_external_macro(cx, hir_id, path.span) {
            return None;
        }

        let point = if self.skipped(path) || !resolves_through_std(cx, path, def_kind, def_id) {
            Point::Blocked
        } else {
            Point::Rewritable {
                hir_id,
                span: last.ident.span,
            }
        };
        Some((first.ident.span, crate_name, point))
    }

    /// Whether a path written through `crate_name` is the rule's
    /// business at all.
    fn covers(&self, crate_name: Symbol) -> bool {
        crate_name == sym::core || (self.config.also_alloc && crate_name == sym::alloc)
    }

    /// Whether the path as written is exempted by `skip_paths`. The key
    /// is built only when there is something to match it against, since
    /// the list is empty in the default config and every path would
    /// otherwise pay for a `String`.
    fn skipped(&self, path: &Path<'_>) -> bool {
        !self.config.skip_paths.is_empty()
            && self
                .config
                .skip_paths
                .contains(&crate::abs_path::canonical_key(&join_path_segments(
                    path.segments,
                )))
    }
}

/// The first *written* segment of a path: the one after the synthetic
/// `PathRoot` that a `::`-rooted path (`::core::fmt::Display`) carries,
/// or the very first otherwise.
fn first_segment<'hir>(path: &Path<'hir>) -> Option<&'hir PathSegment<'hir>> {
    match path.segments {
        [root, next, ..] if root.ident.name == kw::PathRoot => Some(next),
        [first, ..] => Some(first),
        [] => None,
    }
}

/// Whether the path still names `def_id` once its crate segment reads
/// `std` — the check that makes rewriting only that one segment sound.
/// It fails on a path whose suffix does not exist under `std` at all, on
/// one that lands on a *different* item there
/// (`core::panic::PanicInfo`), and, since `std` is then not among the
/// crates in the compilation, on every path in a crate that does not
/// link `std`.
///
/// This resolution is expensive, so it runs last — only once a path has
/// been established as a candidate in every cheaper respect.
fn resolves_through_std(
    cx: &LateContext<'_>,
    path: &Path<'_>,
    def_kind: DefKind,
    def_id: DefId,
) -> bool {
    let ns = match def_kind.ns() {
        Some(Namespace::TypeNS) => PathNS::Type,
        Some(Namespace::ValueNS) => PathNS::Value,
        Some(Namespace::MacroNS) => PathNS::Macro,
        None => PathNS::Arbitrary,
    };
    let mut segments = Vec::with_capacity(path.segments.len());
    segments.push(sym::std);
    segments.extend(
        path.segments
            .iter()
            .skip_while(|segment| segment.ident.name == kw::PathRoot)
            .skip(1)
            .map(|segment| segment.ident.name),
    );
    lookup_path(cx.tcx, ns, &segments).contains(&def_id)
}
