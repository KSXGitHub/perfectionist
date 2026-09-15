use crate::common::DefaultState;
use crate::field_copy::{COPYING_METHODS, FieldCopy, borrowed_form, field_copy};
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags an inherent `as_*` method taking `&self` whose whole body
    /// copies one field of `self` out through `clone`, `to_owned`,
    /// `to_string`, `to_vec`, `to_path_buf`, or `to_os_string`, and asks
    /// for the borrowed form instead: `&str` for a `String` field,
    /// `&Path` for a `PathBuf`, `&OsStr` for an `OsString`, `&[T]` for a
    /// `Vec<T>`, `Option<&T>` for an `Option<T>`, `&T` otherwise.
    ///
    /// A method returning a `Copy` value is left alone, and so is one
    /// that moves a field out rather than copying it: both are free, and
    /// free is what the prefix promises. So is a method of a trait impl,
    /// since the trait fixes its signature, and one produced by a macro.
    ///
    /// Test code is left alone; set `exempt_tests` to `false` to
    /// measure it like any other code.
    ///
    /// ### Why is this bad?
    ///
    /// The Rust API Guidelines give `as_`, `to_` and `into_` distinct
    /// meanings, and `as_` is the free one: a borrowed value viewed as
    /// another borrowed form, costing nothing. A caller reads `as_name()`
    /// as free and may put it in a loop, so an `as_*` that allocates
    /// makes the name a promise the method does not keep. The reader has
    /// no way to see the cost at the call site.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::wrong_self_convention` checks the same three prefixes
    /// against the method's *receiver* -- whether `as_*` takes `&self`.
    /// It does not look at the return type or at what the body costs, so
    /// an `as_*` that takes `&self` and then allocates satisfies it.
    ///
    /// ### Interaction with sibling rules
    ///
    /// `perfectionist::cloning_getter` measures the same body shape on
    /// methods that are not named `as_*` or `to_*`. The two partition
    /// the shape by name, so a method is measured by exactly one of
    /// them. A `to_*` method is measured by neither: that prefix
    /// announces a costly conversion, so its copy is what the name
    /// already promises.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn as_name(&self) -> String {
    ///         self.name.clone()
    ///     }
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn as_name(&self) -> &str {
    ///         &self.name
    ///     }
    /// }
    /// ```
    ///
    /// Or keep the copy and rename it, so the cost is in the name:
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn to_name(&self) -> String {
    ///         self.name.clone()
    ///     }
    /// }
    /// ```
    pub perfectionist::CLONING_AS_CONVERSION,
    Warn,
    "`as_*` method copies a field where its prefix promises a free borrow",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::cloning_as_conversion";

/// The second fix, which the getter rule has no equivalent of: `as_*`
/// carries a promise about cost, and renaming keeps the copy while
/// making it honest.
const RENAME_HELP: &str = "or keep the copy and name it `to_*`, the prefix for a conversion that \
                           costs something, so the call site shows what it pays";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Whether test code is left alone: methods inside a `#[cfg(test)]`
    /// module or an integration-test or benchmark target. Defaults to
    /// `true`.
    exempt_tests: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { exempt_tests: true }
    }
}

pub struct CloningAsConversion {
    config: Config,
    copying_methods: Vec<Symbol>,
}

impl_lint_pass!(CloningAsConversion => [CLONING_AS_CONVERSION]);

impl Register for rule::CloningAsConversion {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[CLONING_AS_CONVERSION]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(CloningAsConversion {
                config: dylint_linting::config_or_default(CONFIG_KEY),
                copying_methods: COPYING_METHODS
                    .iter()
                    .map(|name| Symbol::intern(name))
                    .collect(),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for CloningAsConversion {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _span: Span,
        def_id: LocalDefId,
    ) {
        let Some(FieldCopy {
            method,
            field,
            field_ty,
            def_span,
            ..
        }) = field_copy(cx, kind, decl, body, def_id, &self.copying_methods)
        else {
            return;
        };
        if !method.as_str().starts_with("as_") {
            return;
        }
        if self.config.exempt_tests && item_in_test_code(cx, def_id) {
            return;
        }
        span_lint_and_then(
            cx,
            CLONING_AS_CONVERSION,
            def_span,
            format!("`{method}` copies `self.{field}`, but `as_` promises a free conversion"),
            |diag| {
                diag.help(format!(
                    "return `{}`, which costs nothing and is what the name promises",
                    borrowed_form(cx, field_ty),
                ));
                diag.help(RENAME_HELP);
            },
        );
    }
}
