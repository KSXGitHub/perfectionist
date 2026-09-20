use crate::common::{DefaultState, hir_in_external_macro};
use crate::rule_index::{Register, rule};
use clippy_utils::is_from_proc_macro;
use rustc_hir::{Expr, ExprKind, Node, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

mod availability;
mod config;
mod emit;
mod receiver;
mod setter;

use config::Config;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a `std::process::Command` setter called on an *owned*
    /// command — `arg`, `args`, `env`, `envs`, `env_remove`,
    /// `env_clear`, `current_dir`, `stdin`, `stdout`, `stderr` — and
    /// names the `command_extra::CommandExtra` counterpart that takes
    /// `self` instead of `&mut self`.
    ///
    /// The three stdio setters are generic over `Into<Stdio>` where
    /// their counterparts take a concrete `Stdio`. Where the argument
    /// is already a `Stdio` the counterpart takes it as it stands;
    /// where it is a `File` or a `ChildStdout`, the diagnostic says to
    /// convert it with `.into()`.
    ///
    /// A receiver the by-value form could not consume is left alone:
    /// `CommandExtra` takes `self`, so neither a `&mut Command` nor a
    /// field reached through a reference can adopt it, and the
    /// diagnostic would have no valid fix. By default the lint also
    /// stays silent in a crate that does not depend on `command-extra`,
    /// since the method it names would not exist there; the
    /// `require_command_extra_dependency` knob turns that off.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Both
    /// forms build the same command, and the standard library's form is
    /// not wrong.
    ///
    /// The preference is that a builder whose steps each yield a value
    /// composes with everything else, and one whose steps yield a
    /// borrow composes with nothing. Because `Command`'s setters return
    /// `&mut Command`, a chain of them cannot be returned from a
    /// function, cannot initialise a `let` in one expression, cannot
    /// fill a struct field, and cannot be an accumulator. Each of those
    /// forces a `mut` binding whose only job is to exist until the
    /// settings are done. The by-value form removes that intermediate.
    ///
    /// ### Example
    ///
    /// **Avoid** — the function cannot end in its chain, because the
    /// chain has type `&mut Command`:
    ///
    /// ```rust,ignore
    /// fn lister(dir: &Path) -> Command {
    ///     let mut command = Command::new("ls");
    ///     command
    ///         .current_dir(dir)
    ///         .args(["-l", "-a"])
    ///         .env("LANG", "C");
    ///     command
    /// }
    /// ```
    ///
    /// **Prefer** — one expression, no binding:
    ///
    /// ```rust,ignore
    /// fn lister(dir: &Path) -> Command {
    ///     Command::new("ls")
    ///         .with_current_dir(dir)
    ///         .with_args(["-l", "-a"])
    ///         .with_env("LANG", "C")
    /// }
    /// ```
    ///
    /// ### No automatic fix
    ///
    /// The suggested rename is never applied by `cargo dylint --fix`,
    /// because whether it compiles cannot be decided without
    /// re-typechecking. The by-value form returns `Command` where the
    /// original returned `&mut Command`, and that changed type ripples:
    /// it is rejected where the context wanted the borrow, it moves a
    /// receiver the surrounding code still reads, and — since a trait
    /// method taking `self` is found before an inherent `&mut self` one
    /// — it can silently redirect a *later* call in the same chain to an
    /// extension trait of the author's own, with no compile error to
    /// reveal it.
    ///
    /// So the diagnostic shows the rename where the change looks local
    /// and otherwise describes it in prose, and says which part of the
    /// change the rename does not cover. Over a binding the surrounding
    /// code still reads, finishing it means reassigning that binding or
    /// collapsing it into one chained expression, and which of those
    /// fits depends on what else the body does.
    pub perfectionist::MUTATING_COMMAND_BUILDER,
    Warn,
    "a `std::process::Command` setter taking `&mut self` where `command-extra`'s by-value form exists",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::mutating_command_builder";

pub struct MutatingCommandBuilder {
    require_command_extra_dependency: bool,
    /// Whether this crate declares a dependency on `command_extra`,
    /// memoised on first use. Answering it can walk every
    /// `extern crate` item, and the answer cannot change within a
    /// compilation.
    command_extra_declared: Option<bool>,
}

impl MutatingCommandBuilder {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            require_command_extra_dependency: config.require_command_extra_dependency,
            command_extra_declared: None,
        }
    }

    /// Whether the `CommandExtra` counterpart is writable here.
    ///
    /// An import of the trait counts on its own: a crate that uses the
    /// trait has to import it, and the import resolves to the real
    /// crate even where the declared set cannot see the dependency.
    /// It is asked second because answering it walks the module's
    /// items, where the declared answer is memoised.
    fn suggestion_is_available(&mut self, cx: &LateContext<'_>, call: &Expr<'_>) -> bool {
        !self.require_command_extra_dependency
            || self.command_extra_is_declared(cx)
            || availability::trait_is_imported(cx, call)
    }

    /// [`availability::crate_is_declared`], answered once per
    /// compilation.
    fn command_extra_is_declared(&mut self, cx: &LateContext<'_>) -> bool {
        *self
            .command_extra_declared
            .get_or_insert_with(|| availability::crate_is_declared(cx))
    }
}

impl_lint_pass!(MutatingCommandBuilder => [MUTATING_COMMAND_BUILDER]);

impl Register for rule::MutatingCommandBuilder {
    /// The dependency gate is what keeps this defensible on by
    /// default: at its default the lint stays quiet in a crate that
    /// does not depend on `command-extra`, so it does not press a
    /// third-party dependency on a project that has not met it.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[MUTATING_COMMAND_BUILDER]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(MutatingCommandBuilder::new())));
    }
}

impl<'tcx> LateLintPass<'tcx> for MutatingCommandBuilder {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &Expr<'tcx>) {
        let ExprKind::MethodCall(path_segment, receiver, arguments, _) = expr.kind else {
            return;
        };
        let Some(by_value_form) = setter::by_value_form(path_segment.ident.name) else {
            return;
        };
        if !setter::is_on_an_owned_command(cx, receiver) {
            return;
        }
        if !setter::resolves_to_an_inherent_command_method(cx, expr) {
            return;
        }
        if !receiver::can_be_consumed(cx, receiver) {
            return;
        }
        // The diagnostic span is the method segment alone, narrower
        // than the call it belongs to, so `report_in_external_macro:
        // false` does not cover a derive that stamps a synthesised
        // call with a user-source span.
        if path_segment.ident.span.from_expansion()
            || hir_in_external_macro(cx, expr.hir_id, path_segment.ident.span)
            // A derive that stamps its *whole* output with the driving
            // attribute's span leaves the span-based guard nothing to
            // find, including the enclosing item's `def_span`. Reading
            // the source under the span catches that, as the sibling
            // rules facing the same derive shape do.
            || is_from_proc_macro(cx, expr)
        {
            return;
        }
        // Last of the gates rather than first: it can need whether the
        // trait is imported, and that needs the call, which every check
        // above has already narrowed to `Command`'s own setters.
        if !self.suggestion_is_available(cx, expr) {
            return;
        }
        emit::violation(
            cx,
            emit::Violation {
                hir_id: expr.hir_id,
                method_span: path_segment.ident.span,
                std_form: path_segment.ident.name,
                by_value_form,
                conversion: setter::argument_conversion(cx, expr, arguments),
                names_generic_arguments: path_segment
                    .args
                    .is_some_and(|arguments| !arguments.args.is_empty()),
                receiver_is_a_temporary: receiver::produces_a_temporary(receiver),
                // The position has to be written where the call is. A
                // macro taking an expression and using it twice gives
                // both uses the caller's span, so a rename shown for the
                // use in an accepting position would be written over the
                // other use as well -- and that other one may be exactly
                // the borrow the original returned. So a position the
                // macro's own body supplies is declined however many
                // times the macro uses the expression, which costs a
                // rendered rewrite rather than a wrong one; a position
                // among the caller's own tokens is not.
                position_takes_it: accepting_position(cx, expr)
                    .is_some_and(|span| !span.from_expansion()),
                // Which remedy to name: the crate is absent from the
                // manifest, or present but not imported here. Only the
                // gate knows the first, and only with the gate turned
                // off can it happen. Scoped to the module, so a macro
                // stamping one written call into two modules can still
                // earn a diagnostic apiece -- each naming what its own
                // module needs.
                remedy: match (
                    self.command_extra_is_declared(cx),
                    availability::trait_is_imported(cx, expr),
                ) {
                    (_, true) => None,
                    (true, false) => Some("bring `command_extra::CommandExtra` into scope here"),
                    (false, false) => Some(
                        "add `command-extra` to this crate's dependencies, then import \
                         `CommandExtra`",
                    ),
                },
            },
        );
    }
}

/// The span of the position `call`'s value lands in, where that
/// position accepts the owned `Command` the by-value form returns and
/// the original yielded a `&mut Command`.
///
/// This decides how the diagnostic *reads*, not whether anything is
/// applied: nothing is. Two positions qualify. A discarded statement
/// value constrains nothing, so the change is sound there outright. A
/// method receiver usually accepts it, because autoref supplies the
/// borrow the next method wants -- but not always: a method found on
/// `&mut Command` itself, or a bound only `&mut Command` satisfies
/// (`.into()` being the everyday one), is lost once the receiver
/// becomes a `Command`. Separating those needs the typechecker, which
/// is why the rename is only ever shown and never applied.
///
/// Every other position -- a `let`, a call argument, a struct field --
/// may or may not accept the change, and which is likewise the
/// typechecker's answer, so the diagnostic stays with prose there.
///
/// The span comes back with the answer because the caller has to know
/// whether the position was written where the call is, and because
/// which of the two reasons applies is what the reader is told.
fn accepting_position(cx: &LateContext<'_>, call: &Expr<'_>) -> Option<Span> {
    match cx.tcx.parent_hir_node(call.hir_id) {
        Node::Expr(parent) => match parent.kind {
            ExprKind::MethodCall(_, parent_receiver, ..) => {
                (parent_receiver.hir_id == call.hir_id).then_some(parent.span)
            }
            _ => None,
        },
        Node::Stmt(statement) => match statement.kind {
            StmtKind::Semi(_) => Some(statement.span),
            _ => None,
        },
        _ => None,
    }
}
