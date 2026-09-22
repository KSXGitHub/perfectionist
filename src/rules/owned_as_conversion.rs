mod owned_ty;

use crate::common::DefaultState;
use crate::field_copy::{
    COPYING_METHODS, Eligible, FieldCopy, borrowed_form, eligible_method, field_copy,
};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use owned_ty::owned_return;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::print::ForceTrimmedGuard;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags an inherent `as_*` method taking `&self` and nothing else
    /// that hands the caller a value of their own, where the prefix
    /// promised a borrow. Two shapes say so, and the second is what the
    /// diagnostic can suggest a fix for:
    ///
    /// 1. **The body copies one field of `self`** out through `clone`,
    ///    `to_owned`, `to_string`, `to_vec`, `to_path_buf`, or
    ///    `to_os_string`, reproducing that field's own type. A borrow of
    ///    the field would have served, so the rule names it: `&str` for
    ///    a `String` field, `&Path` for a `PathBuf`, `&OsStr` for an
    ///    `OsString`, `&CStr` for a `CString`, `&[T]` for a `Vec<T>`,
    ///    whatever the inner type borrows as under an `Option` or a
    ///    `Box`, and `&T` otherwise.
    /// 2. **The return type owns a heap allocation** — a `String`, a
    ///    `Vec<T>`, a `PathBuf`, an `OsString`, a `CString`, a `Box<T>`,
    ///    one of the standard maps, sets or queues, or any of those
    ///    under an `Option` or a `Result`. Here there is nothing to
    ///    borrow instead, because the value was built rather than
    ///    copied, so renaming is the whole fix.
    ///
    /// What is reported is ownership, not allocation. Whether a body
    /// allocates cannot be read off a signature, so the rule never
    /// claims it: `as_key(&self) -> String` returning `String::new()`
    /// allocates nothing and is still a value the caller must drop.
    ///
    /// A `Copy` return type is left alone throughout — handing one back
    /// by value is free, which is what the prefix promises. So is a type
    /// carrying a lifetime, `Cow<'_, str>` among them, since it is free
    /// to borrow from the receiver, and so is an `Rc` or an `Arc` field:
    /// cloning one bumps a refcount rather than copying what it points
    /// at, and a caller keeping the handle has to own one.
    ///
    /// A method of a trait impl is left alone, since the trait fixes its
    /// signature, and so is one produced by a macro.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. The Rust
    /// API Guidelines give `as_`, `to_` and `into_` distinct meanings,
    /// and `as_` is the free one: a borrowed value viewed as another
    /// borrowed form, free. A caller reads `as_name()` as free and may
    /// put it in a loop, so an `as_*` that hands back something owned
    /// makes the name a promise the method does not keep. The reader has
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
    /// conversion that costs something, so handing back an owned value
    /// is what those names already promise. A name that is none of the
    /// three and that the getter rule does not read as a getter — one
    /// naming no field, with nothing in `getter_name_patterns` admitting
    /// it — is left alone by both as well.
    ///
    /// ### Example
    ///
    /// **Avoid:** a field copied out, where a borrow would have served
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
    /// **Avoid:** a value built rather than copied, where no borrow of
    /// `self` could stand in for it
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn as_age_label(&self) -> String {
    ///         self.age.to_string()
    ///     }
    /// }
    /// ```
    ///
    /// **Prefer:** the body kept and the prefix dropped, so the name
    /// admits what it costs
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn to_age_label(&self) -> String {
    ///         self.age.to_string()
    ///     }
    /// }
    /// ```
    pub perfectionist::OWNED_AS_CONVERSION,
    Warn,
    "`as_*` method hands back an owned value where its prefix promises a free borrow",
    report_in_external_macro: false
}

/// The prefix this rule measures. `cloning_getter` reads the same
/// string out of `crate::getter_name_patterns::CONVERSION_PREFIXES` to
/// decide what it will never call a getter, so the two rules' partition
/// rests on it appearing in both places. The unit test beside this file
/// is what notices if it stops.
const AS_PREFIX: &str = "as_";

/// The prefix a flagged method should have worn instead: the one that
/// announces a conversion the caller pays for.
const COSTLY_PREFIX: &str = "to_";

const CONFIG_KEY: &str = "perfectionist::owned_as_conversion";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct OwnedAsConversion {
    copying_methods: Vec<Symbol>,
}

impl_lint_pass!(OwnedAsConversion => [OWNED_AS_CONVERSION]);

impl Register for rule::OwnedAsConversion {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[OWNED_AS_CONVERSION]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
            Box::new(OwnedAsConversion {
                copying_methods: COPYING_METHODS
                    .iter()
                    .map(|name| Symbol::intern(name))
                    .collect(),
            })
        }));
    }
}

/// The help that is available wherever the rule fires: the method keeps
/// what it does and loses the prefix that misdescribes it.
fn rename_help(method: Symbol) -> String {
    format!(
        "rename it `{}`, the prefix for a conversion that costs something, so the call site \
         shows what it pays",
        method.as_str().replacen(AS_PREFIX, COSTLY_PREFIX, 1),
    )
}

impl<'tcx> LateLintPass<'tcx> for OwnedAsConversion {
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
        // The field copy is the shape that can be answered with a
        // borrow, so it is tried first and reported on its own terms.
        // Consulting the shared recogniser rather than restating it is
        // also what keeps a method from being reported by both this
        // rule and `cloning_getter`.
        if let Some(FieldCopy {
            method,
            field,
            field_ty,
            def_span,
            ..
        }) = field_copy(cx, kind, decl, body, def_id, &self.copying_methods)
        {
            span_lint_and_then(
                cx,
                OWNED_AS_CONVERSION,
                def_span,
                format!("`{method}` copies `self.{field}`, but `as_` promises a free conversion"),
                |diag| {
                    diag.help(format!(
                        "either stop it copying: return `{}`, which is free and is what the name \
                         promises",
                        borrowed_form(cx, field_ty),
                    ));
                    diag.help(format!(
                        "or stop it being an `as_*`: {}",
                        rename_help(method),
                    ));
                },
            );
            return;
        }
        let Some(Eligible { method, def_span }) = eligible_method(cx, kind, decl, body, def_id)
        else {
            return;
        };
        // The signature's own late-bound regions have to go before the
        // type is asked anything. `skip_binder` would leave them
        // escaping, and `is_copy` panics rather than answering for a
        // type in that state; an `Option<&String>` return is enough to
        // trip it. Erasing them is sound here because the predicate
        // only asks whether a region is present, never which one.
        let output = cx
            .tcx
            .instantiate_bound_regions_with_erased(
                cx.tcx.fn_sig(def_id).instantiate_identity().skip_norm_wip(),
            )
            .output();
        if !owned_return(cx, output) {
            return;
        }
        // Print the type trimmed to its final segment, so the message
        // reads `String` rather than `std::string::String`.
        let _trimmed = ForceTrimmedGuard::new();
        span_lint_and_then(
            cx,
            OWNED_AS_CONVERSION,
            def_span,
            format!("`{method}` returns an owned `{output}`, but `as_` promises a free conversion"),
            |diag| {
                diag.note(
                    "the value is built rather than borrowed, so no borrow of `self` would serve \
                     in its place",
                );
                diag.help(rename_help(method));
            },
        );
    }
}

#[cfg(test)]
mod tests;
