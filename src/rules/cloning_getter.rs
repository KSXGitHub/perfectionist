use crate::common::{DefaultState, resolve_string_set};
use crate::exempt_prefix::ExemptPrefix;
use crate::field_copy::{COPYING_METHODS, FieldCopy, borrowed_form, field_copy, has_field};
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::Ty;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol};
use std::collections::BTreeSet;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a getter — an inherent method taking `&self` whose whole
    /// body is one field of `self` copied out through `clone`,
    /// `to_owned`, `to_string`, `to_vec`, `to_path_buf`, or
    /// `to_os_string` — and asks for the borrowed form instead: `&str`
    /// for a `String` field, `&Path` for a `PathBuf`, `&OsStr` for an
    /// `OsString`, `&[T]` for a `Vec<T>`, `Option<&T>` for an
    /// `Option<T>`, `&T` otherwise.
    ///
    /// The call has to reproduce the field's own type for a borrow to
    /// serve in its place. So a `Copy` field is left alone — returning
    /// it by value is the borrowed form's equal — and so is a call that
    /// renders the field rather than copying it, such as `to_string` on
    /// a numeric field, where no borrow of the field is a `String`.
    ///
    /// What counts as a getter is decided by the first of these that
    /// applies:
    ///
    /// 1. `to_*`, `into_*` and `as_*` are conversions, never getters.
    ///    The first two announce that they cost something, so the copy is
    ///    part of what the name promises; `as_*` promises the opposite,
    ///    and `perfectionist::cloning_as_conversion` measures it.
    /// 2. A name carrying an exempt prefix -- `clone_*` and `cloned_*`
    ///    by default, configurable -- is never a getter either: the name
    ///    already tells a caller the copy is there.
    /// 3. `get_*` is a getter.
    /// 4. A method named for a field of `self` is a getter.
    /// 5. Any other name is a getter only where `measure_unmatched_names`
    ///    says so.
    ///
    /// Every clause also requires the `&self` receiver and no other
    /// parameter.
    ///
    /// A method of a trait impl is left alone, since the trait fixes
    /// its signature, and so is a method produced by a macro.
    ///
    /// Test code is left alone; set `exempt_tests` to `false` to
    /// measure it like any other code.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A
    /// getter that clones decides for every caller that they wanted an
    /// owned copy, and most did not: they compare, print, or pass the
    /// value on. The borrowed form serves every caller, costs nothing,
    /// and leaves the one caller that does need ownership to say so
    /// with a `.to_owned()` at the call site, where the reader can see
    /// the copy. It also stops the getter from advertising the field's
    /// representation: a `&str` getter can later be backed by a
    /// `Box<str>` or an interned symbol without a caller changing.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::clone_on_copy` is what catches the `.clone()` on a
    /// `Copy` field that this rule leaves alone. No Clippy lint looks at
    /// what a getter returns.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn first_name(&self) -> String {
    ///         self.first_name.clone()
    ///     }
    ///     fn middle_name(&self) -> Option<String> {
    ///         self.middle_name.clone()
    ///     }
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn first_name(&self) -> &str {
    ///         &self.first_name
    ///     }
    ///     fn middle_name(&self) -> Option<&str> {
    ///         self.middle_name.as_deref()
    ///     }
    /// }
    /// ```
    pub perfectionist::CLONING_GETTER,
    Warn,
    "getter returns an owned copy of a field where a borrow would serve",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::cloning_getter";

/// Name prefixes that keep a method out of the rule whatever its body
/// does, because the name already tells a caller the copy is there.
/// Unlike the conversion prefixes, which the API guidelines define,
/// these are a convention a project chooses, so they are configurable.
const DEFAULT_EXEMPT_PREFIXES: &[&str] = &["clone_", "cloned_"];

/// The second of the two remedies. A violation is a method that both
/// clones *and* is a getter, so dropping either half resolves it: the
/// first help drops the clone, this one drops the getter.
///
/// `to_*` is the rename that does it without trading one complaint for
/// another. The other two prefixes leave getter-hood behind as well,
/// but each carries a promise the copy would then break, and a rule to
/// match: `perfectionist::cloning_as_conversion` for `as_*`, which says
/// the call is free.
///
/// It carries the test for choosing it, in the shape the sibling rules
/// use: a call site that copies the borrow straight back is what says
/// the callers wanted the owned value all along.
const RENAME_HELP: &str = "or stop it being a getter: rename it `to_*`, the prefix for a \
                           conversion that costs something, which is the right shape when every \
                           call site would copy the borrow straight back";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Additional name prefixes that keep a method out of the rule,
    /// whatever its body does and whatever `measure_unmatched_names`
    /// says. Merged with the built-in defaults (`["clone_", "cloned_"]`);
    /// empty by default. Each entry ends in `_` and is not one of the
    /// conversion prefixes `as_`, `into_`, `to_`, which the rule exempts
    /// whatever this says; anything else is rejected at config-parse
    /// time.
    extra_exempt_prefixes: Vec<ExemptPrefix>,
    /// Prefixes to drop from the exempt set, even if they appear in the
    /// built-in defaults or in `extra_exempt_prefixes`. Empty by default;
    /// checked after the merge, so this knob always wins. Each entry is
    /// shaped as `extra_exempt_prefixes` requires.
    ignore_exempt_prefixes: Vec<ExemptPrefix>,
    /// Whether a method whose name neither starts with `get_` nor matches
    /// a field of `self` is still treated as a getter. With this off, such
    /// a method is left alone however its body reads. A conversion prefix
    /// -- `to_*`, `into_*`, `as_*` -- and an exempt prefix are never
    /// getters whatever this says. Defaults to `false`.
    measure_unmatched_names: bool,
    /// Whether test code is left alone: getters inside a `#[cfg(test)]`
    /// module or an integration-test or benchmark target. Defaults to
    /// `true`.
    exempt_tests: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            extra_exempt_prefixes: Vec::new(),
            ignore_exempt_prefixes: Vec::new(),
            measure_unmatched_names: false,
            exempt_tests: true,
        }
    }
}

pub struct CloningGetter {
    config: Config,
    exempt_prefixes: BTreeSet<String>,
    copying_methods: Vec<Symbol>,
}

impl_lint_pass!(CloningGetter => [CLONING_GETTER]);

impl Register for rule::CloningGetter {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[CLONING_GETTER]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
            let exempt_prefixes = resolve_string_set(
                DEFAULT_EXEMPT_PREFIXES,
                config
                    .extra_exempt_prefixes
                    .iter()
                    .cloned()
                    .map(String::from)
                    .collect(),
                config
                    .ignore_exempt_prefixes
                    .iter()
                    .cloned()
                    .map(String::from)
                    .collect(),
            );
            Box::new(CloningGetter {
                config,
                exempt_prefixes,
                copying_methods: COPYING_METHODS
                    .iter()
                    .map(|name| Symbol::intern(name))
                    .collect(),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for CloningGetter {
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
            self_ty,
            def_span,
        }) = field_copy(cx, kind, decl, body, def_id, &self.copying_methods)
        else {
            return;
        };
        if !self.is_getter(method, self_ty) {
            return;
        }
        if self.config.exempt_tests && item_in_test_code(cx, def_id) {
            return;
        }
        span_lint_and_then(
            cx,
            CLONING_GETTER,
            def_span,
            format!("getter `{method}` returns an owned copy of `self.{field}`"),
            |diag| {
                diag.help(format!(
                    "either stop it cloning: return `{}`, and let a caller that needs ownership \
                     copy at the call site",
                    borrowed_form(cx, field_ty),
                ));
                diag.help(RENAME_HELP);
            },
        );
    }
}

impl CloningGetter {
    /// Whether `method` on `self_ty` is a getter, by the definition this
    /// rule measures. The clauses are tried in order, and the first that
    /// applies decides:
    ///
    /// 1. `to_*`, `into_*` and `as_*` are conversions, never getters.
    ///    Each prefix carries its own promise about cost and ownership.
    /// 2. A name carrying an exempt prefix is never a getter either. The
    ///    default roster is `clone_` and `cloned_`, which say outright
    ///    that the method copies; unlike clause 1's prefixes, which the
    ///    API guidelines define, this one is a project's own convention
    ///    and so is configurable.
    /// 3. `get_*` is a getter.
    /// 4. A method named for a field of `self` is a getter.
    /// 5. Any other name is a getter only where `measure_unmatched_names`
    ///    says so.
    ///
    /// Every clause also requires the single `&self` receiver, which the
    /// caller has already established.
    fn is_getter(&self, method: Symbol, self_ty: Ty<'_>) -> bool {
        let name = method.as_str();
        if name.starts_with("to_") || name.starts_with("into_") || name.starts_with("as_") {
            return false;
        }
        if self
            .exempt_prefixes
            .iter()
            .any(|prefix| name.starts_with(prefix.as_str()))
        {
            return false;
        }
        if name.starts_with("get_") {
            return true;
        }
        if has_field(self_ty, method) {
            return true;
        }
        self.config.measure_unmatched_names
    }
}
