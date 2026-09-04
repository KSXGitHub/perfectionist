use crate::common::DefaultState;
use crate::rule_index::{Register, rule};
use crate::test_code::fn_in_test_code;
use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{Pat, PatKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol};
use std::collections::BTreeSet;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Counts the distinct names a function or method body binds —
    /// through `let`, `if let`, `while let`, `let ... else`, `for`,
    /// match arms, and the parameters of closures it contains — and
    /// flags the body when the count is above `max_bindings` (default
    /// `15`).
    ///
    /// A name counts once however many times it is bound, so
    /// shadowing (`let input = input.trim();`) is free. A name that
    /// begins with `_` is not counted, nor is a binding produced by a
    /// macro expansion or a compiler desugaring, nor are the function's
    /// own parameters. A nested function is counted on its own, not as
    /// part of the function that contains it. A function that is itself
    /// produced by a macro is not counted.
    ///
    /// Test code is counted like any other code; set
    /// `test_code_exception` to leave it alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Every
    /// local name is a value the reader has to keep track of, and a
    /// body that needs many of them at once is usually several steps
    /// written out in sequence, each with its own intermediates. Unlike
    /// a branch or a loop, such a body has no control flow for a
    /// complexity measure to see, so it passes those checks however
    /// long it grows. The cap pushes each step into a function of its
    /// own, whose signature then names the few values that cross
    /// between them, or pushes related intermediates into a struct.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::too_many_arguments` caps the names that enter a
    /// function through its signature; this rule caps the ones the
    /// body introduces. `clippy::too_many_lines` caps the body's length
    /// instead, which a wrapped chain or a long literal can exhaust
    /// without introducing a single name.
    ///
    /// ### Example
    ///
    /// **Avoid:** one body that reads, parses, validates, and reports
    ///
    /// ```rust,ignore
    /// fn load(path: &Path) -> Result<Config, Error> {
    ///     let raw = fs::read_to_string(path)?;
    ///     let document = toml::from_str::<Document>(&raw)?;
    ///     let registry = document.registry.unwrap_or_default();
    ///     let registry_url = Url::parse(&registry)?;
    ///     let store_dir = document.store_dir.map(PathBuf::from);
    ///     let store_dir_exists = store_dir.as_deref().is_some_and(Path::exists);
    ///     let concurrency = document.concurrency.unwrap_or(16);
    ///     let concurrency_clamped = concurrency.clamp(1, 64);
    ///     // ... eight more
    ///     Ok(Config { registry_url, store_dir, concurrency: concurrency_clamped, .. })
    /// }
    /// ```
    ///
    /// **Prefer:** one function per step
    ///
    /// ```rust,ignore
    /// fn load(path: &Path) -> Result<Config, Error> {
    ///     let document = read_document(path)?;
    ///     Ok(Config {
    ///         registry_url: parse_registry(&document)?,
    ///         store_dir: resolve_store_dir(&document),
    ///         concurrency: clamp_concurrency(&document),
    ///     })
    /// }
    /// ```
    pub perfectionist::TOO_MANY_LOCAL_BINDINGS,
    Warn,
    "function body binds more distinct local names than the configured maximum",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::too_many_local_bindings";

/// The same ceiling Pylint's `too-many-locals` ships with.
const DEFAULT_MAX_BINDINGS: usize = 15;

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// The most distinct local names a function body may bind without
    /// being flagged. Defaults to `15`.
    max_bindings: usize,
    /// Whether test code is left alone: functions inside a
    /// `#[cfg(test)]` module, `#[test]` functions, and everything in
    /// an integration-test or benchmark target. Defaults to `false`,
    /// so a test is held to the same limit as the code it exercises.
    test_code_exception: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_bindings: DEFAULT_MAX_BINDINGS,
            test_code_exception: false,
        }
    }
}

pub struct TooManyLocalBindings {
    config: Config,
}

impl_lint_pass!(TooManyLocalBindings => [TOO_MANY_LOCAL_BINDINGS]);

impl Register for rule::TooManyLocalBindings {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[TOO_MANY_LOCAL_BINDINGS]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(TooManyLocalBindings {
                config: dylint_linting::config_or_default(CONFIG_KEY),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for TooManyLocalBindings {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _span: Span,
        def_id: LocalDefId,
    ) {
        // A closure's bindings belong to the function that contains it.
        let (FnKind::ItemFn(ident, ..) | FnKind::Method(ident, ..)) = kind else {
            return;
        };
        let def_span = cx.tcx.def_span(def_id);
        if def_span.from_expansion() {
            return;
        }
        if self.config.test_code_exception && fn_in_test_code(cx, def_id) {
            return;
        }
        let count = count_local_bindings(cx.tcx, body);
        if count <= self.config.max_bindings {
            return;
        }
        let max = self.config.max_bindings;
        let name = ident.name;
        let noun = if count == 1 { "name" } else { "names" };
        let message = format!(
            "function `{name}` binds {count} distinct local {noun}, above the limit of {max}",
        );
        span_lint_and_help(
            cx,
            TOO_MANY_LOCAL_BINDINGS,
            def_span,
            message,
            None,
            "split the body into one function per step, or gather related values into a struct",
        );
    }
}

/// The number of distinct names bound anywhere in `body` other than
/// its own parameters.
fn count_local_bindings<'tcx>(tcx: TyCtxt<'tcx>, body: &'tcx hir::Body<'tcx>) -> usize {
    let mut collector = BindingCollector {
        tcx,
        names: BTreeSet::new(),
    };
    collector.visit_expr(body.value);
    collector.names.len()
}

struct BindingCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    names: BTreeSet<Symbol>,
}

impl<'tcx> Visitor<'tcx> for BindingCollector<'tcx> {
    type NestedFilter = nested_filter::OnlyBodies;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_pat(&mut self, pat: &'tcx Pat<'tcx>) {
        // A binding the compiler makes up for a desugaring — the `iter`
        // of a `for` loop, the `val` and `residual` of a `?` — carries a
        // dummy span rather than an expansion span.
        if let PatKind::Binding(_, _, ident, _) = pat.kind
            && !ident.span.from_expansion()
            && !ident.span.is_dummy()
            && !ident.name.as_str().starts_with('_')
        {
            self.names.insert(ident.name);
        }
        intravisit::walk_pat(self, pat);
    }
}
