use crate::common::{DefaultState, hir_in_external_macro};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_hir_and_then;
use clippy_utils::is_from_proc_macro;
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind, Node};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

mod availability;
mod config;
mod receiver;
mod setter;

use config::Config;
use setter::Conversion;

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
    /// stays silent in a crate that has not loaded `command-extra`,
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
    /// So the diagnostic shows the rename where the receiver is a value
    /// the expression produced, and describes it where the receiver is a
    /// place, since there the rename alone would not compile. Finishing
    /// it there means reassigning the binding or collapsing it into one
    /// chained expression, which depends on what else the body does.
    pub perfectionist::MUTATING_COMMAND_BUILDER,
    Warn,
    "a `std::process::Command` setter taking `&mut self` where `command-extra`'s by-value form exists",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::mutating_command_builder";

pub struct MutatingCommandBuilder {
    require_command_extra_dependency: bool,
    /// Whether a crate named `command_extra` is among the ones the
    /// compiler loaded, memoised on first use. Answering it walks
    /// every loaded crate, and the answer cannot change within a
    /// compilation.
    command_extra_loaded: Option<bool>,
}

impl MutatingCommandBuilder {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            require_command_extra_dependency: config.require_command_extra_dependency,
            command_extra_loaded: None,
        }
    }

    /// Whether the `CommandExtra` counterpart is writable here.
    fn suggestion_is_available(&mut self, cx: &LateContext<'_>) -> bool {
        // Answered either way, even with the gate off: the diagnostic
        // reads it to decide which remedy to name.
        let loaded = self.command_extra_is_loaded(cx);
        !self.require_command_extra_dependency || loaded
    }

    /// [`availability::crate_is_loaded`], answered once per compilation.
    fn command_extra_is_loaded(&mut self, cx: &LateContext<'_>) -> bool {
        *self
            .command_extra_loaded
            .get_or_insert_with(|| availability::crate_is_loaded(cx))
    }
}

impl_lint_pass!(MutatingCommandBuilder => [MUTATING_COMMAND_BUILDER]);

impl Register for rule::MutatingCommandBuilder {
    /// The dependency gate is what keeps this defensible on by
    /// default: at its default the lint stays quiet in a crate where
    /// the compiler never loaded `command-extra`, so it does not press
    /// a third-party dependency on a project that has not met it. A
    /// crate reaching it only transitively is the gap in that -- the
    /// gate asks what was loaded, not what was declared.
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
        // First, because after the first call it is one bool read,
        // where everything below it walks types or places.
        if !self.suggestion_is_available(cx) {
            return;
        }
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
        let std_form = path_segment.ident.name;
        // Show the rename only where it is the literal whole change:
        // the receiver is consumable without disturbing anyone, and the
        // context takes the owned `Command` the by-value form returns.
        let conversion = setter::argument_conversion(cx, expr, arguments);
        // A rename plus an `.into()` is not a rename, so it is never
        // shown as one.
        let rename_is_the_whole_change = conversion == Conversion::None
            && receiver::produces_a_temporary(receiver)
            && feeds_a_method_receiver(cx, expr);
        // Which remedy to name: the crate is absent from the manifest,
        // or present but not imported here. Only the gate knows the
        // first, and only with the gate turned off can it happen.
        let remedy = match (
            self.command_extra_is_loaded(cx),
            availability::trait_is_imported(cx, expr),
        ) {
            (_, true) => None,
            (true, false) => Some("bring `command_extra::CommandExtra` into scope here"),
            (false, false) => {
                Some("add `command-extra` to this crate's dependencies, then import `CommandExtra`")
            }
        };
        span_lint_hir_and_then(
            cx,
            MUTATING_COMMAND_BUILDER,
            expr.hir_id,
            path_segment.ident.span,
            format!("`Command::{std_form}` takes `&mut self`, so it cannot yield the command"),
            |diagnostic| {
                let advice = match conversion {
                    Conversion::None => format!(
                        "use `CommandExtra::{by_value_form}`, which takes `self` and returns \
                         `Self`, keeping the whole construction in expression position",
                    ),
                    Conversion::IntoNeeded => format!(
                        "use `CommandExtra::{by_value_form}`, which takes `self` and returns \
                         `Self`; it takes the argument by value, so convert it with `.into()`",
                    ),
                };
                match rename_is_the_whole_change {
                    // The rename is the whole rewrite, so show it.
                    true => {
                        diagnostic.span_suggestion(
                            path_segment.ident.span,
                            advice,
                            by_value_form,
                            Applicability::MaybeIncorrect,
                        );
                    }
                    // The receiver is a place the surrounding code still
                    // holds, so the rename alone moves it. Printing one
                    // would show a rewrite that does not compile.
                    false => {
                        diagnostic.help(advice);
                        diagnostic.help(
                            "the receiver outlives this call, so finish the change by \
                             reassigning it or by collapsing the binding into one chained \
                             expression",
                        );
                    }
                }
                if let Some(remedy) = remedy {
                    diagnostic.help(remedy);
                }
            },
        );
    }
}

/// Whether `call`'s value is the receiver of another method call.
///
/// This decides how the diagnostic *reads*, not whether anything is
/// applied: nothing is. A method receiver is the one position that
/// takes the owned `Command` the by-value form returns where the
/// original yielded a `&mut Command`, so it is the one position where
/// printing the bare rename shows something that compiles.
fn feeds_a_method_receiver(cx: &LateContext<'_>, call: &Expr<'_>) -> bool {
    matches!(
        cx.tcx.parent_hir_node(call.hir_id),
        Node::Expr(Expr {
            kind: ExprKind::MethodCall(_, parent_receiver, ..),
            ..
        }) if parent_receiver.hir_id == call.hir_id,
    )
}
