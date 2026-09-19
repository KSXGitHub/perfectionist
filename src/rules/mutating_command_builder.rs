use crate::common::{DefaultState, hir_in_external_macro};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_hir_and_then;
use clippy_utils::res::MaybeDef;
use rustc_hir::def::Res;
use rustc_hir::{Expr, ExprKind, QPath};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::adjustment::Adjust;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Symbol;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a `std::process::Command` setter called on an *owned*
    /// command — `arg`, `args`, `env`, `envs`, `env_remove`,
    /// `env_clear`, `current_dir`, `stdin`, `stdout`, `stderr` — and
    /// names the `command_extra::CommandExtra` counterpart that takes
    /// `self` instead of `&mut self`.
    ///
    /// A receiver the by-value form could not take ownership of is left
    /// alone: `CommandExtra` takes `self`, so neither a `&mut Command`
    /// nor a field reached through a reference can adopt it, and the
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
    /// The diagnostic is advice rather than a rewrite, because
    /// renaming the method in place is not always a valid fix. The
    /// by-value form returns `Command` where the original returned
    /// `&mut Command`, so a call whose value is consumed as
    /// `&mut Command` — an argument to `fn configure(&mut Command)`,
    /// say — stops compiling when only its name changes. Where the
    /// call is a statement on a `mut` binding, the fix is to collapse
    /// the binding into a single chained expression, which depends on
    /// what else the body does with it. Both are the author's to make.
    pub perfectionist::MUTATING_COMMAND_BUILDER,
    Warn,
    "a `std::process::Command` setter taking `&mut self` where `command-extra`'s by-value form exists",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::mutating_command_builder";

/// The crate name `command-extra` compiles under, as the compiler
/// spells it rather than as Cargo does.
const COMMAND_EXTRA_CRATE: &str = "command_extra";

/// `std::process::Command`'s `rustc_diagnostic_item` name. Not among
/// the pre-interned `rustc_span::sym` constants, so it is interned on
/// use, as `needless_borrowed_parameters` does for the same reason.
const COMMAND_DIAGNOSTIC_ITEM: &str = "Command";

/// The user-facing configuration shape, deserialised from the
/// `["perfectionist::mutating_command_builder"]` table of
/// `dylint.toml`.
#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Whether to stay silent in a crate that has not loaded
    /// `command-extra`. Defaults to `true`: without the crate the
    /// suggested method does not exist, so the diagnostic would name
    /// something the author cannot write. Set it to `false` in a
    /// workspace that adds the dependency per-crate and wants the
    /// lint to say where it is still missing.
    require_command_extra_dependency: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            require_command_extra_dependency: true,
        }
    }
}

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
        if !self.require_command_extra_dependency {
            return true;
        }
        *self.command_extra_loaded.get_or_insert_with(|| {
            let wanted = Symbol::intern(COMMAND_EXTRA_CRATE);
            cx.tcx
                .crates(())
                .iter()
                .any(|&krate| cx.tcx.crate_name(krate) == wanted)
        })
    }
}

impl_lint_pass!(MutatingCommandBuilder => [MUTATING_COMMAND_BUILDER]);

impl Register for rule::MutatingCommandBuilder {
    /// The dependency gate is what keeps this defensible on by
    /// default: at its default the lint cannot fire in a crate that
    /// has not already reached for `command-extra`, so it never
    /// pushes a third-party dependency on anyone.
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
        let ExprKind::MethodCall(path_segment, receiver, _, _) = expr.kind else {
            return;
        };
        let Some(by_value_form) = by_value_form(path_segment.ident.name) else {
            return;
        };
        // Unadjusted, so an autoref inserted for the `&mut self`
        // signature does not hide an owned receiver -- and so a
        // receiver that is genuinely a `&mut Command` keeps its
        // reference and fails the check.
        if !cx
            .typeck_results()
            .expr_ty(receiver)
            .is_diag_item(cx, Symbol::intern(COMMAND_DIAGNOSTIC_ITEM))
        {
            return;
        }
        if !receiver_can_be_consumed(cx, receiver) {
            return;
        }
        // The diagnostic span is the method segment alone, narrower
        // than the call it belongs to, so `report_in_external_macro:
        // false` does not cover a derive that stamps a synthesised
        // call with a user-source span.
        if hir_in_external_macro(cx, expr.hir_id, path_segment.ident.span) {
            return;
        }
        if !self.suggestion_is_available(cx) {
            return;
        }
        let std_form = path_segment.ident.name;
        span_lint_hir_and_then(
            cx,
            MUTATING_COMMAND_BUILDER,
            expr.hir_id,
            path_segment.ident.span,
            format!("`Command::{std_form}` takes `&mut self`, so it cannot yield the command"),
            |diagnostic| {
                diagnostic.help(format!(
                    "use `CommandExtra::{by_value_form}`, which takes `self` and returns `Self`, \
                     keeping the whole construction in expression position",
                ));
            },
        );
    }
}

/// Whether the by-value form could take ownership of `receiver`.
///
/// The type alone does not answer this. A `Command` field reached
/// through `&mut self` has type `Command` with no reference in sight,
/// and `self.command.with_arg(..)` on it is `E0507: cannot move out of
/// `self.command` which is behind a mutable reference`. So walk the
/// place expression and accept only what the caller owns outright.
/// Anything unrecognised is treated as not consumable, which costs a
/// missed diagnostic rather than an unfixable one.
fn receiver_can_be_consumed(cx: &LateContext<'_>, receiver: &Expr<'_>) -> bool {
    match receiver.kind {
        // A local binding, `self` taken by value among them.
        ExprKind::Path(QPath::Resolved(None, path)) => matches!(path.res, Res::Local(_)),
        // A field of something the caller owns, so long as reaching it
        // does not pass through a reference -- which is what an
        // autoderef adjustment on the base records.
        ExprKind::Field(base, _) => {
            !cx.typeck_results()
                .expr_adjustments(base)
                .iter()
                .any(|adjustment| matches!(adjustment.kind, Adjust::Deref(_)))
                && receiver_can_be_consumed(cx, base)
        }
        // A value this expression produced, which is a temporary
        // nobody else holds a claim on.
        ExprKind::Call(..)
        | ExprKind::MethodCall(..)
        | ExprKind::Struct(..)
        | ExprKind::Block(..)
        | ExprKind::If(..)
        | ExprKind::Match(..) => true,
        _ => false,
    }
}

/// The `command_extra::CommandExtra` counterpart of a
/// `std::process::Command` setter, or `None` for any other method --
/// `Command::new`, which is not a setter, and the spawning methods,
/// which have no counterpart and take `&mut self` legitimately.
fn by_value_form(std_form: Symbol) -> Option<&'static str> {
    Some(match std_form.as_str() {
        "arg" => "with_arg",
        "args" => "with_args",
        "env" => "with_env",
        "envs" => "with_envs",
        "env_remove" => "without_env",
        "env_clear" => "with_no_env",
        "current_dir" => "with_current_dir",
        "stdin" => "with_stdin",
        "stdout" => "with_stdout",
        "stderr" => "with_stderr",
        _ => return None,
    })
}
