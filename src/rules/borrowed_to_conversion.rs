use crate::common::{DefaultState, unwrap_block};
use crate::field_copy::{Eligible, eligible_method};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{BorrowKind, Expr, ExprKind, QPath, UnOp};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::{self, Ty};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, kw, sym};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags an inherent `to_*` method taking `&self` and nothing else
    /// that hands back a borrow — `&T`, or an `Option` or `Result` of
    /// one — and does nothing to earn it: the body is a field of
    /// `self`, a borrow of one, or a single `as_*` view of one. It
    /// asks for the `as_*` prefix instead.
    ///
    /// Left alone:
    ///
    /// - A body that does work on the way to the borrow — a call, a
    ///   `match`, a validation. That is what `to_` is for.
    /// - A `to_*` with another parameter, which is converting
    ///   something more than `self`.
    /// - A trait impl's method. The trait fixes the signature.
    /// - A method produced by a macro.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. The
    /// Rust API Guidelines sort the three conversion prefixes by what
    /// they cost: `as_` is the free one, `to_` the expensive one.
    /// Either may hand back a borrow — `Path::to_str` validates UTF-8
    /// and returns `Option<&str>`, and the guidelines say outright
    /// that calling that one `as_str` would be wrong. It is the
    /// reverse they leave no room for: a `to_*` that costs nothing,
    /// whose name asks a caller to avoid in a loop what they could
    /// have had for free.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::wrong_self_convention` checks the same three prefixes
    /// against the method's *receiver* — whether `to_*` takes `&self`.
    /// It does not look at what the body costs, so a `to_*` that takes
    /// `&self` and only borrows satisfies it.
    ///
    /// ### Interaction with sibling rules
    ///
    /// `perfectionist::owned_as_conversion` is this rule's mirror: it
    /// flags an `as_*` that hands back an owned value, where this
    /// flags a `to_*` that hands back a borrow for nothing. Each
    /// rule's fix is the other's prefix.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn to_name(&self) -> &str {
    ///         &self.name
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
    pub perfectionist::BORROWED_TO_CONVERSION,
    Warn,
    "`to_*` method hands back a borrow for nothing where its prefix promises a costly conversion",
    report_in_external_macro: false
}

/// The second of the two remedies. A violation both borrows and
/// carries the `to_` prefix, so dropping either half resolves it.
const OWNED_HELP: &str = "or stop it borrowing: hand back a value of the caller's own, where one \
                          really is needed";

/// The prefix this rule measures.
const TO_PREFIX: &str = "to_";

/// The prefix a flagged method should have worn instead, and the one a
/// method call in the body may be trusted to be free: the guidelines
/// give both the same meaning.
const AS_PREFIX: &str = "as_";

const CONFIG_KEY: &str = "perfectionist::borrowed_to_conversion";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct BorrowedToConversion;

impl_lint_pass!(BorrowedToConversion => [BORROWED_TO_CONVERSION]);

impl Register for rule::BorrowedToConversion {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[BORROWED_TO_CONVERSION]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
            Box::new(BorrowedToConversion)
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for BorrowedToConversion {
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
        // comparison; `eligible_method` re-lexes the method's source text
        // to rule out a proc macro. Ask the cheap question first, so only
        // a `to_*` method pays for the expensive one.
        let FnKind::Method(ident, _) = kind else {
            return;
        };
        if !ident.name.as_str().starts_with(TO_PREFIX) {
            return;
        }
        let Some(Eligible { method, def_span }) = eligible_method(cx, kind, decl, body, def_id)
        else {
            return;
        };
        // Erase the signature's late-bound regions before asking about
        // the return type. The predicate only asks whether a reference
        // is there, never which region it carries, and leaving them
        // bound hands an escaping-bound-vars type to anything that
        // wraps one in a dummy binder -- `is_copy`, for one, which the
        // mirror rule's return-type predicate opens with.
        let output = cx
            .tcx
            .instantiate_bound_regions_with_erased(
                cx.tcx.fn_sig(def_id).instantiate_identity().skip_norm_wip(),
            )
            .output();
        if !returns_a_borrow(cx, output) {
            return;
        }
        if !costless_body(unwrap_block(body.value)) {
            return;
        }
        let suggested = method.as_str().replacen(TO_PREFIX, AS_PREFIX, 1);
        span_lint_and_then(
            cx,
            BORROWED_TO_CONVERSION,
            def_span,
            format!("`{method}` only borrows, but `to_` promises a costly conversion"),
            |diag| {
                diag.help(format!(
                    "either stop it being a `to_*`: rename it `{suggested}`, the prefix for a \
                     conversion that costs nothing, so the call site shows it is free",
                ));
                diag.help(OWNED_HELP);
            },
        );
    }
}

/// Whether `ty` hands the caller a borrow rather than a value of their
/// own: a reference, or an `Option` or `Result` of one, which are as
/// free as the reference inside them.
fn returns_a_borrow<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    if ty.is_ref() {
        return true;
    }
    let ty::Adt(adt, args) = ty.kind() else {
        return false;
    };
    let did = adt.did();
    if !cx.tcx.is_diagnostic_item(sym::Option, did) && !cx.tcx.is_diagnostic_item(sym::Result, did)
    {
        return false;
    }
    args.types().next().is_some_and(Ty::is_ref)
}

/// Whether the body hands over a borrow it already had, doing nothing
/// on the way: a place inside `self`, or a single `as_*` call on one.
/// Anything else — a call, a `match`, a block with a statement in it —
/// is work, and work is what `to_` announces.
fn costless_body(expr: &Expr<'_>) -> bool {
    if let ExprKind::MethodCall(segment, receiver, [], _) = expr.kind {
        // An `as_*` call is free by the same guideline this rule reads,
        // so trusting the name here is trusting what the rule enforces.
        return segment.ident.name.as_str().starts_with(AS_PREFIX) && borrows_self(receiver);
    }
    borrows_self(expr)
}

/// Whether `expr` names a place inside `self`, or borrows one:
/// `self.name`, `&self.name`, `&self.inner.name`, `&*self.boxed`.
fn borrows_self(mut expr: &Expr<'_>) -> bool {
    loop {
        expr = match expr.kind {
            ExprKind::AddrOf(BorrowKind::Ref, _, inner) => inner,
            ExprKind::Field(base, _) => base,
            ExprKind::Unary(UnOp::Deref, base) => base,
            ExprKind::Path(QPath::Resolved(None, path)) => {
                return matches!(path.segments, [segment] if segment.ident.name == kw::SelfLower);
            }
            _ => return false,
        };
    }
}
