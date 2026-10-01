use crate::command_extra::{is_the_trait, trait_is_imported};
use crate::common::{DefaultState, hir_in_external_macro};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::source::snippet;
use clippy_utils::sugg::Sugg;
use clippy_utils::{is_from_proc_macro, sym};
use rustc_errors::Applicability;
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

mod folder;
mod receiver;
mod setter;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a `fold` over a singular `command_extra::CommandExtra`
    /// setter where the trait declares the plural that does the same
    /// thing, and names the plural. The pairs are `with_arg` against
    /// `with_args`, `with_env` against `with_envs`, and `without_env`
    /// against `without_envs`.
    ///
    /// The folder is matched by what it resolves to, so every spelling
    /// of the setter is one shape, a closure that only forwards to it
    /// included.
    ///
    /// The `with_env` pair is flagged only when the resolved `with_envs`
    /// accepts what the fold iterates. That bound has changed between
    /// `command-extra` releases, so the rule never names a plural the
    /// code cannot call.
    ///
    /// The fold's receiver has to be a place expression followed by at
    /// most one argument-less method call. `VARS`, `list.iter()` and
    /// `self.names.into_iter()` qualify; a longer one is left alone.
    ///
    /// A fold in the body of the plural itself is left alone, because
    /// there the fold is how the plural is implemented.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. The fold
    /// is correct and produces an identical `Command`.
    ///
    /// The preference is that a fold over a builder setter makes the
    /// reader reconstruct an operation the API already names. `fold` is a
    /// general-purpose combinator, so meeting one obliges the reader to
    /// work out what is being accumulated and in what order before they
    /// can see that the answer is "these variables are removed".
    /// `without_envs` says that outright, and the receiver stops being an
    /// accumulator threaded through a closure.
    ///
    /// There is a second, smaller reason: the fold form has more places
    /// to get wrong. A fold whose closure swaps its accumulator and item,
    /// or returns the wrong one of the two, compiles in some shapes and
    /// silently builds the wrong command. The plural has no such surface.
    ///
    /// `command-extra` defines `with_args` as
    /// `args.into_iter().fold(self, Self::with_arg)`, so a hand-rolled
    /// fold is not an alternative to the plural: it is the plural's own
    /// body, inlined one level up from where the library already wrote
    /// it.
    ///
    /// ### Applicability
    ///
    /// The suggestion moves the receiver past the initial value, so a
    /// fix is applied only where that cannot change what either of them
    /// sees: the receiver runs no code but the standard library's, over
    /// the standard library's types, and reads nothing the initial value
    /// writes. Elsewhere the suggestion is advice, as it also is where
    /// the trait is not in scope at the call site.
    ///
    /// The suggestion keeps the receiver's call unless the plural makes
    /// that same call itself.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// INHERITED_VARS
    ///     .iter()
    ///     .fold(Command::new("cargo"), CommandExtra::without_env)
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// Command::new("cargo").without_envs(INHERITED_VARS.iter())
    /// ```
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// flags
    ///     .iter()
    ///     .fold(Command::new("ls"), |command, flag| command.with_arg(flag))
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// Command::new("ls").with_args(flags.iter())
    /// ```
    pub perfectionist::FOLDED_COMMAND_SETTER,
    Warn,
    "fold over a singular `CommandExtra` setter re-implements the plural",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::folded_command_setter";

/// What a reader has to settle before applying the suggestion by hand
/// where the receiver is not reorderable. Why the two orders differ is on
/// [`receiver::Shape::reorderable`].
const REORDERS: &str = "the fold evaluates the receiver before the initial value and the \
                        suggestion evaluates them the other way round, so apply it only where \
                        neither reads what the other writes";

/// What a reader has to do first where the trait is not in scope at the
/// call site. Conditional, because the imports [`trait_is_imported`]
/// does not answer for leave this line redundant rather than wrong.
const NEEDS_THE_IMPORT: &str = "the plural is a `CommandExtra` method, which resolves only where \
                                the trait is in scope; add \
                                `use command_extra::CommandExtra;` if it is not";

/// What a reader has to settle where an impl that may apply to the
/// accumulator writes the plural's body. Why that withholds the fix is
/// on [`setter::overrides_the_plural`].
const OVERRIDDEN: &str = "an impl of `CommandExtra` may write its own body for the plural, which \
                          the suggestion would run in place of the fold; apply it only where the \
                          two agree";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct FoldedCommandSetter;

impl_lint_pass!(FoldedCommandSetter => [FOLDED_COMMAND_SETTER]);

impl Register for rule::FoldedCommandSetter {
    /// The trigger names `CommandExtra`'s own setters, so it can only
    /// fire in code that already calls them. No dependency gate is
    /// needed, unlike the sibling rule's.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[FOLDED_COMMAND_SETTER]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(FoldedCommandSetter)));
    }
}

impl<'tcx> LateLintPass<'tcx> for FoldedCommandSetter {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::MethodCall(segment, receiver, [initial, folder], _) = expr.kind else {
            return;
        };
        if segment.ident.name != sym::fold {
            return;
        }
        let Some(fold) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
            return;
        };
        // `fold`'s name is shared with inherent methods of other types,
        // so the trait it belongs to is what identifies it.
        if !cx
            .tcx
            .trait_of_assoc(fold)
            .is_some_and(|trait_id| cx.tcx.is_diagnostic_item(sym::Iterator, trait_id))
        {
            return;
        }
        let Some(singular) = folder::resolves_to(cx, folder) else {
            return;
        };
        // Resolving the setter is what proves the accumulator implements
        // `CommandExtra`: `fold`'s `B` is the `Self` of the impl the
        // setter resolved in, or the fold does not type-check. So the
        // accumulator's type is never asked, and the rule reaches
        // whatever a release implements the trait for.
        let Some(trait_id) = cx.tcx.trait_of_assoc(singular) else {
            return;
        };
        if !is_the_trait(cx, trait_id) {
            return;
        }
        let Some(replacement) = setter::replacement_for(cx.tcx.item_name(singular)) else {
            return;
        };
        let Some(plural_id) = setter::declares(cx, trait_id, replacement.plural) else {
            return;
        };
        if setter::inside_the_plural(cx, expr, plural_id) {
            return;
        }

        if replacement.splits_item && !setter::item_fits(cx, plural_id, receiver) {
            return;
        }
        let Some(shape) = receiver::shape(cx, receiver, initial) else {
            return;
        };
        // An initial value that diverges never reaches the fold, so the
        // plural would run where the fold did not: measured turning a
        // `return`-as-accumulator into a command whose environment is
        // stripped. `Sugg`'s bracketing does not cover these, and there
        // is nothing to advise about code rustc already calls
        // unreachable.
        if cx.typeck_results().expr_ty(initial).is_never() {
            return;
        }
        // The plural is named, not resolved, so an inherent method of the
        // accumulator's own type would take the call instead -- and need
        // not take the same arguments.
        if setter::shadowed_by_an_inherent_method(cx, initial, replacement.plural) {
            return;
        }
        // A derive that stamps its whole expansion with the driving
        // attribute's span defeats both `report_in_external_macro:
        // false` and `hir_in_external_macro`, which read spans;
        // `is_from_proc_macro` reads the source text under the span
        // instead, and is what keeps
        // `ui/folded_command_setter_proc_macro.rs` silent.
        if hir_in_external_macro(cx, expr.hir_id, expr.span) || is_from_proc_macro(cx, expr) {
            return;
        }
        // Neither of those covers a `macro_rules!` of the linted crate's
        // own. The suggestion replaces the span it is reported at while
        // its text is read from the spans of three sub-expressions, so
        // inside a macro body it rewrites the *definition* with text
        // spliced from a call site: measured turning a macro that
        // removed environment variables into one that adds arguments,
        // and pasting a caller's local into a body where hygiene cannot
        // resolve it. Two invocations also earn two suggestions at one
        // span, which no fixer can reconcile.
        if [expr.span, initial.span, shape.argument]
            .iter()
            .any(|span| span.from_expansion())
        {
            return;
        }
        // The plural is named rather than resolved, so the rewritten
        // call reaches it only where the trait is in scope -- and a
        // *path* folder needs no import of its own, so a fold can name
        // the setter while the module cannot name the method. Measured:
        // a machine-applicable `E0599` without this.
        emit(
            cx,
            expr,
            initial,
            singular,
            plural_id,
            replacement.plural,
            &shape,
        );
    }
}

/// Report the fold, handing the fix over only where none of the reasons
/// to withhold it holds, and naming each reason that does.
fn emit<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    initial: &'tcx Expr<'tcx>,
    singular: DefId,
    plural_id: DefId,
    plural: &str,
    shape: &receiver::Shape,
) {
    let accumulator = cx.typeck_results().expr_ty(initial);
    let overridden = setter::overrides_the_plural(cx, plural_id, accumulator);
    let ambiguous_with = setter::another_trait_declaring(cx, plural_id, accumulator);
    let in_scope = trait_is_imported(cx, expr);
    let applicability = if shape.reorderable && in_scope && !overridden && ambiguous_with.is_none()
    {
        Applicability::MachineApplicable
    } else {
        Applicability::Unspecified
    };
    let initial_text = match initial.kind {
        // A struct literal reads as the start of a block where the call
        // lands in a scrutinee or a condition, so it keeps brackets
        // everywhere. rustc does not warn about them elsewhere.
        ExprKind::Struct(..) => format!("({})", snippet(cx, initial.span, "..")),
        // The initial value becomes a method-call receiver, so one that
        // binds looser has to keep its own brackets: `*boxed` spliced raw
        // reads as `*boxed.plural(..)`, which derefs the *result*.
        _ => Sugg::hir(cx, initial, "..").maybe_paren().to_string(),
    };
    let suggestion = format!(
        "{initial_text}.{plural}({})",
        snippet(cx, shape.argument, ".."),
    );
    span_lint_and_then(
        cx,
        FOLDED_COMMAND_SETTER,
        expr.span,
        format!(
            "this fold over `{}` re-implements `{}`",
            cx.tcx.item_name(singular),
            plural,
        ),
        |diagnostic| {
            diagnostic.span_suggestion(
                expr.span,
                format!("use `{}`", plural),
                suggestion,
                applicability,
            );
            if !shape.reorderable {
                diagnostic.help(REORDERS);
            }
            if !in_scope {
                diagnostic.help(NEEDS_THE_IMPORT);
            }
            if overridden {
                diagnostic.help(OVERRIDDEN);
            }
            if let Some(other) = ambiguous_with {
                diagnostic.help(format!(
                    "`{}` also declares `{}` for this type, so wherever both traits are in \
                     scope the suggestion is ambiguous; call the plural through the path \
                     of `CommandExtra` there",
                    cx.tcx.def_path_str(other),
                    plural,
                ));
            }
        },
    );
}
