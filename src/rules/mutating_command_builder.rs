use crate::cargo_target::crate_target;
use crate::common::{DefaultState, hir_in_external_macro};
use crate::rule_index::{Register, rule};
use crate::test_code::in_test_code;
use clippy_utils::is_from_proc_macro;
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, ExprKind, Node, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

mod availability;
mod config;
mod emit;
mod fix;
mod probe;
mod receiver;
mod setter;

use config::{Config, RequiredDeclaration};
use emit::Landing;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a `std::process::Command` setter called on an *owned*
    /// command — `arg`, `args`, `env`, `envs`, `env_remove`,
    /// `env_clear`, `current_dir`, `stdin`, `stdout`, `stderr` — and
    /// names the `command_extra::CommandExtra` counterpart that takes
    /// `self` instead of `&mut self`.
    ///
    /// A receiver it could not take ownership of — a `&mut Command`, or
    /// a field reached through one — is left alone. So is a crate that
    /// neither depends on `command-extra` nor belongs to a workspace
    /// declaring it; `command_extra_dependency` sets how far the lint
    /// looks for that declaration. Where `command-extra` is in the
    /// build at all, a chain is left alone unless every setter in it
    /// has a counterpart there — the by-value forms arrived over
    /// several releases, and a chain is ported whole or not at all. In
    /// a `--test` build of a library or binary only its test code is
    /// flagged; the rest is judged by the build that ships it.
    ///
    /// A fix is applied where the whole change is known: a chain is
    /// rewritten at once, never in part. Elsewhere the diagnostic
    /// describes the change, and may show the rename without applying
    /// it.
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
    pub perfectionist::MUTATING_COMMAND_BUILDER,
    Warn,
    "a `std::process::Command` setter taking `&mut self` where `command-extra`'s by-value form exists",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::mutating_command_builder";

pub struct MutatingCommandBuilder {
    command_extra_dependency: RequiredDeclaration,
    /// Whether this crate declares a dependency on `command_extra`,
    /// memoised on first use. Answering it can walk every
    /// `extern crate` item, and the answer cannot change within a
    /// compilation.
    command_extra_declared: Option<bool>,
    /// The same, for the surrounding workspace's own table.
    /// [`crate::cargo_manifest`] reads and parses that file once per
    /// process; this saves the scan over its dependency table.
    workspace_declared: Option<bool>,
    /// The `CommandExtra` traits the compilation loaded, memoised on
    /// first use. Finding them walks the crate graph, and the answer
    /// cannot change within a compilation.
    command_extra_traits: Option<Vec<DefId>>,
}

impl MutatingCommandBuilder {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            command_extra_dependency: config.command_extra_dependency,
            command_extra_declared: None,
            workspace_declared: None,
            command_extra_traits: None,
        }
    }

    /// Whether every counterpart the chain `call` heads would be
    /// renamed to is one the loaded `CommandExtra` has.
    ///
    /// The whole chain or none of it, as [`fix`] rewrites it and for
    /// the reason its module doc gives: a link short of a counterpart
    /// cannot be ported at all, and renaming the others around it
    /// would leave it calling a std setter on a receiver the change
    /// has made owned. So the head stands down with it.
    ///
    /// The walk ends where the rewrite's does, because nothing past
    /// there is renamed.
    fn chain_counterparts_are_declared<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        call: &'tcx Expr<'tcx>,
        by_value_form: &'static str,
    ) -> bool {
        if !self.counterpart_is_declared(cx, by_value_form) {
            return false;
        }
        let mut tail = call;
        while let Node::Expr(parent) = cx.tcx.parent_hir_node(tail.hir_id) {
            let ExprKind::MethodCall(method, receiver, ..) = parent.kind else {
                break;
            };
            if receiver.hir_id != tail.hir_id {
                break;
            }
            let Some(by_value_form) = setter::by_value_form(method.ident.name) else {
                break;
            };
            if !setter::resolves_to_an_inherent_command_method(cx, parent) {
                break;
            }
            if !self.counterpart_is_declared(cx, by_value_form) {
                return false;
            }
            tail = parent;
        }
        true
    }

    /// Whether the counterpart the diagnostic would name is one every
    /// loaded `CommandExtra` has.
    ///
    /// Every one of them, because which a lookup would otherwise pick
    /// is load order; a counterpart only some of them declare cannot
    /// be named without knowing which the code under lint resolves to.
    ///
    /// `true` where nothing loaded the trait, which falls out of
    /// asking an empty set rather than being cased for. That is a
    /// crate not using it yet, still free to resolve a version that
    /// has the counterpart; one already using it is held to the
    /// version it has.
    fn counterpart_is_declared(&mut self, cx: &LateContext<'_>, by_value_form: &str) -> bool {
        self.command_extra_traits
            .get_or_insert_with(|| availability::loaded_traits(cx))
            .iter()
            .all(|command_extra| {
                availability::declares_the_counterpart(cx, *command_extra, by_value_form)
            })
    }

    /// Whether the `CommandExtra` counterpart is near enough to hand
    /// to be named, at the reach `command_extra_dependency` asks for.
    ///
    /// An import of the trait counts on its own: a crate that uses the
    /// trait has to import it. It is asked last because answering it
    /// walks the module's items, where the other answers are memoised.
    fn suggestion_is_available(&mut self, cx: &LateContext<'_>, call: &Expr<'_>) -> bool {
        match self.command_extra_dependency {
            RequiredDeclaration::Unchecked => return true,
            RequiredDeclaration::Workspace => {
                if self.workspace_declares_command_extra() {
                    return true;
                }
            }
            RequiredDeclaration::Crate => {}
        }
        self.command_extra_is_declared(cx) || availability::trait_is_imported(cx, call)
    }

    /// [`availability::crate_is_declared`], answered once per
    /// compilation.
    fn command_extra_is_declared(&mut self, cx: &LateContext<'_>) -> bool {
        *self
            .command_extra_declared
            .get_or_insert_with(|| availability::crate_is_declared(cx))
    }

    /// [`availability::workspace_declares_the_package`], answered once
    /// per compilation.
    fn workspace_declares_command_extra(&mut self) -> bool {
        *self
            .workspace_declared
            .get_or_insert_with(availability::workspace_declares_the_package)
    }
}

impl_lint_pass!(MutatingCommandBuilder => [MUTATING_COMMAND_BUILDER]);

impl Register for rule::MutatingCommandBuilder {
    /// The dependency gate is what keeps this defensible on by
    /// default: at its default the lint stays quiet in a project that
    /// has declared `command-extra` nowhere, so it does not press a
    /// third-party dependency on one that has not met it.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[MUTATING_COMMAND_BUILDER]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(MutatingCommandBuilder::new())));
    }
}

impl<'tcx> LateLintPass<'tcx> for MutatingCommandBuilder {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
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
        // A library's own code is judged by the build that ships it.
        // `--all-targets` compiles that code twice, and the `--test`
        // build's dependencies are the wider set, so a dependency
        // reaching only that build would earn advice to import it
        // where the shipping build has nothing -- `E0432` the moment
        // the advice is taken. The shipping build asks the same code
        // itself wherever it can be asked, so nothing is lost.
        //
        // An integration test, a benchmark and an example have no
        // shipping build to defer to, and their own helpers carry
        // neither mark of test code, so they are asked as they stand.
        if LintContext::sess(cx).opts.test
            && !crate_target(cx).is_separate_target()
            && !in_test_code(cx.tcx, expr.hir_id)
        {
            return;
        }
        // Naming a counterpart the resolved `command-extra` does not
        // have gives advice that cannot be followed and a rewrite that
        // does not compile.
        if !self.chain_counterparts_are_declared(cx, expr, by_value_form) {
            return;
        }
        // Last of the gates rather than first: it can need whether the
        // trait is imported, and that needs the call, which every check
        // above has already narrowed to `Command`'s own setters.
        if !self.suggestion_is_available(cx, expr) {
            return;
        }
        let landing = landing(cx, expr);
        let conversion = setter::argument_conversion(cx, expr, arguments);
        let names_generic_arguments = path_segment
            .args
            .is_some_and(|arguments| !arguments.args.is_empty());
        let receiver_is_a_temporary = receiver::produces_a_temporary(receiver);
        let trait_is_imported = availability::trait_is_imported(cx, expr);
        emit::violation(
            cx,
            emit::Violation {
                hir_id: expr.hir_id,
                method_span: path_segment.ident.span,
                std_form: path_segment.ident.name,
                by_value_form,
                conversion,
                names_generic_arguments,
                receiver_is_a_temporary,
                landing,
                // `(false, false)` is reachable whenever the gate
                // passed on something else -- the workspace's own
                // table, or nothing at all. Scoping `trait_is_imported`
                // to the module lets a macro stamping one written call
                // into two modules earn a diagnostic apiece, each
                // naming what its module needs.
                remedy: match (self.command_extra_is_declared(cx), trait_is_imported) {
                    (_, true) => None,
                    (true, false) => Some("bring `command_extra::CommandExtra` into scope here"),
                    // Which table depends on the Cargo target: a build
                    // script is compiled against `[build-dependencies]`
                    // alone, a test or a benchmark also against
                    // `[dev-dependencies]`. Naming the condition rather
                    // than resolving it keeps the advice right for
                    // every target, including the `[target.*]` forms a
                    // list would miss.
                    (false, false) => Some(
                        "add `command-extra` to the dependencies table this target is \
                         compiled against (e.g. `[dependencies]`, `[dev-dependencies]`, \
                         `[build-dependencies]`), then import `CommandExtra`",
                    ),
                },
                rewrite: fix::rewrite(
                    cx,
                    expr,
                    &fix::Inputs {
                        by_value_form,
                        conversion,
                        receiver_is_a_temporary,
                        trait_is_imported,
                        names_generic_arguments,
                    },
                ),
            },
        );
    }
}

/// Where `call`'s value lands, as far as this can tell without the
/// typechecker.
///
/// This decides how the diagnostic *reads* wherever no rewrite was
/// built, which is the only case it is consulted in. A discarded
/// statement value constrains nothing, so the change is sound there
/// outright. A method receiver usually accepts it, because autoref
/// supplies the borrow the next method wants -- but not always: a
/// method found on `&mut Command` itself, or a bound only
/// `&mut Command` satisfies (`.into()` being the everyday one), is lost
/// once the receiver becomes a `Command`. Separating those needs the
/// typechecker, which is why a rename rendered on this answer alone is
/// `MaybeIncorrect`.
///
/// Every other position -- a `let`, a call argument, a struct field --
/// may or may not accept the change, and which is likewise the
/// typechecker's answer, so the diagnostic stays with prose there.
fn landing(cx: &LateContext<'_>, call: &Expr<'_>) -> Landing {
    // A macro taking an expression and using it twice gives both uses
    // the caller's span, so a rename shown for the use in an accepting
    // position would be written over the other use as well -- and that
    // other one may be exactly the borrow the original returned. So a
    // position the macro's own body supplies is declined however many
    // times the macro uses the expression, which costs a rendered
    // rewrite rather than a wrong one; a position among the caller's
    // own tokens is not.
    // A call the macro's own body wrote arrives here carrying the
    // caller's token for its method name, because that is what an
    // `$ident` fragment holds. Renaming that token rewrites every other
    // use of it in the body -- `stringify!($method)` among them, which
    // compiles and changes what the command is run with. `fix` declines
    // such a call outright, and the rendered rename has to as well.
    if call.span.from_expansion() {
        return Landing::Unknown;
    }
    match cx.tcx.parent_hir_node(call.hir_id) {
        Node::Expr(parent) if parent.span.from_expansion() => Landing::Unknown,
        Node::Expr(parent) => match parent.kind {
            ExprKind::MethodCall(next, parent_receiver, ..)
                if parent_receiver.hir_id == call.hir_id =>
            {
                match probe::finds_a_trait_method(cx, parent, next.ident.name) {
                    true => Landing::MovesALaterCall(next.ident.name),
                    false => Landing::TakesTheRename,
                }
            }
            _ => Landing::Unknown,
        },
        Node::Stmt(statement) => match statement.kind {
            StmtKind::Semi(_) if !statement.span.from_expansion() => Landing::TakesTheRename,
            _ => Landing::Unknown,
        },
        _ => Landing::Unknown,
    }
}
