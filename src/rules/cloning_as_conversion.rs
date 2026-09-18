use crate::common::DefaultState;
use crate::field_copy::{COPYING_METHODS, FieldCopy, borrowed_form, field_copy};
use crate::rule_index::{Register, rule};
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
    /// `&Path` for a `PathBuf`, `&OsStr` for an `OsString`, `&CStr` for a
    /// `CString`, `&[T]` for a `Vec<T>`, whatever the inner type borrows
    /// as under an `Option` or a `Box`, and `&T` otherwise.
    ///
    /// The call has to reproduce the field's own type for a borrow to
    /// serve in its place. So a `Copy` field is left alone — handing one
    /// back by value is free, which is what the prefix promises — and so
    /// is a call that renders the field rather than copying it, such as
    /// `to_string` on a numeric field, where no borrow of the field is a
    /// `String`.
    ///
    /// An `Rc` or an `Arc` field is left alone too: cloning one bumps a
    /// refcount rather than copying what it points at, so a borrow saves
    /// nothing, and a caller that keeps the handle has to own one.
    ///
    /// Only a method taking `&self` and nothing else is measured. A
    /// method of a trait impl is left alone, since the trait fixes its
    /// signature, and so is one produced by a macro.
    ///
    /// ### Why is this bad?
    ///
    /// The Rust API Guidelines give `as_`, `to_` and `into_` distinct
    /// meanings, and `as_` is the free one: a borrowed value viewed as
    /// another borrowed form, free. A caller reads `as_name()` as free
    /// and may put it in a loop, so an `as_*` that allocates makes the
    /// name a promise the method does not keep. The reader has
    /// no way to see the cost at the call site.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::wrong_self_convention` checks the same three prefixes
    /// against the method's *receiver* — whether `as_*` takes `&self`.
    /// It does not look at the return type or at what the body costs, so
    /// an `as_*` that takes `&self` and then allocates satisfies it.
    ///
    /// ### Interaction with sibling rules
    ///
    /// `perfectionist::cloning_getter` measures the same body shape, but
    /// only on a name it reads as a getter, and it never reads one as a
    /// getter where the name starts with `as_`, `to_` or `into_`. So no
    /// method is measured by both.
    ///
    /// Some are measured by neither. `to_*` and `into_*` announce a
    /// conversion that costs something, so a copy is what those names
    /// already promise. A name that is none of the three and that the
    /// getter rule does not read as a getter — one naming no field, with
    /// nothing in `getter_name_patterns` admitting it — is left alone by
    /// both as well.
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
    /// **Prefer:** the copy dropped, so the call is as free as the name
    /// says
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn as_name(&self) -> &str {
    ///         &self.name
    ///     }
    /// }
    /// ```
    ///
    /// **Prefer:** or the copy kept and the prefix dropped, so the name
    /// admits what it costs
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

/// The second of the two remedies. A violation is a method that both
/// copies *and* carries the `as_` prefix, so dropping either half
/// resolves it: the first help drops the copy, this one drops the
/// prefix, keeping the copy and making the name admit it.
const RENAME_HELP: &str = "or stop it being an `as_*`: rename it `to_*`, the prefix for a \
                           conversion that costs something, so the call site shows what it pays";

/// The prefix this rule measures. `cloning_getter` reads the same
/// string out of `crate::getter_name_patterns::CONVERSION_PREFIXES` to
/// decide what it will never call a getter, so the two rules' partition
/// rests on it appearing in both places. The unit test beside this file
/// is what notices if it stops.
const AS_PREFIX: &str = "as_";

const CONFIG_KEY: &str = "perfectionist::cloning_as_conversion";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct CloningAsConversion {
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
            let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
            Box::new(CloningAsConversion {
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
        // The name decides this rule on its own, and costs a string
        // comparison; `field_copy` re-lexes the method's source text to
        // rule out a proc macro. Ask the cheap question first, so only
        // an `as_*` method pays for the expensive one.
        let FnKind::Method(ident, _) = kind else {
            return;
        };
        if !ident.name.as_str().starts_with(AS_PREFIX) {
            return;
        }
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
        span_lint_and_then(
            cx,
            CLONING_AS_CONVERSION,
            def_span,
            format!("`{method}` copies `self.{field}`, but `as_` promises a free conversion"),
            |diag| {
                diag.help(format!(
                    "either stop it copying: return `{}`, which is free and is what the name \
                     promises",
                    borrowed_form(cx, field_ty),
                ));
                diag.help(RENAME_HELP);
            },
        );
    }
}

#[cfg(test)]
mod tests;
