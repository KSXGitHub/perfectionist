use crate::command_extra::{imports, is_the_trait};
use crate::common::{DefaultState, hir_in_external_macro};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::source::{snippet_opt, snippet_with_applicability};
use clippy_utils::sugg::Sugg;
use clippy_utils::{is_from_proc_macro, span_extract_comments, sym};
use rustc_errors::Applicability;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, ExprKind, QPath};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::print::CratePrefixGuard;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, kw};

mod folder;
mod initial;
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
    /// Every spelling of the setter counts, a closure that only forwards
    /// to it included.
    ///
    /// The rule names a plural only where the code can call it.
    ///
    /// The fold's receiver has to be a place expression followed by at
    /// most one argument-less method call. `VARS`, `list.iter()` and
    /// `self.names.into_iter()` qualify; a longer one is left alone.
    ///
    /// A fold inside the plural's own body is left alone where the
    /// suggestion would make the plural call itself.
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
    /// The fold form also has more places to get wrong. A fold whose
    /// closure swaps its accumulator and item, or returns the wrong one of
    /// the two, compiles in some shapes and silently builds the wrong
    /// command. The plural has no such surface.
    ///
    /// `command-extra` defines each plural as the fold of its singular, so
    /// a hand-rolled fold is not an alternative to the plural but its
    /// body, inlined.
    ///
    /// ### Applicability
    ///
    /// The fix is applied only where it is known to compile and to behave
    /// as the fold did. Elsewhere the suggestion is advice, with
    /// a help line saying what to check first.
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
                        their order does not matter";

/// What a reader has to do first where the trait is not in scope at the
/// call site, with the trait's path in place of `{}`. Conditional, because
/// an import [`imports`] does not see, a glob among them, leaves this line
/// redundant rather than wrong.
const NEEDS_THE_IMPORT: &str = "the plural is a `CommandExtra` method, which resolves only where \
                                the trait is in scope; add `use {};` if it is not";

/// What a reader has to settle where an impl that may apply to the
/// accumulator writes the plural's body. Why that withholds the fix is
/// on [`setter::overrides_the_plural`].
const OVERRIDDEN: &str = "an impl of `CommandExtra` may write its own body for the plural, which \
                          the suggestion would run in place of the fold; apply it only where the \
                          two agree";

/// What a reader has to do first where the initial value may take its
/// type from the fold. Why that withholds the fix is on
/// [`initial::fixes_its_own_type`].
const UNTYPED: &str = "the initial value may take its type from the fold, and as the plural's \
                       receiver it would have none; name its type first";

/// What a reader has to settle where the fold names a type the
/// suggestion drops: in a turbofish on `fold`, or on the closure folder's
/// item, as [`folder::annotates_the_item`] says. Either can be all that
/// fixes the iterator's item type.
const DROPS_A_TYPE: &str = "the fold names a type the suggestion drops; apply it only where the \
                            iterator fixes its item type without it";

/// What a reader has to carry over by hand where the fold holds a comment
/// the suggestion leaves out.
const DROPS_A_COMMENT: &str = "the suggestion keeps the text of the initial value and the \
                               receiver only, so move the fold's other comments by hand";

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
    /// fire in code that already calls them, and needs no dependency
    /// gate.
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
        // `CommandExtra`: `fold`'s `B` is the `Self` the setter is called
        // with, or the fold does not type-check. So no list of
        // accumulator types gates the trigger, and the rule reaches
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
        if !setter::bounds_are_known(cx, plural_id) {
            return;
        }
        if setter::inside_the_plural(cx, expr, initial, plural_id) {
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
        if setter::shadowed_by_an_inherent_method(cx, initial, plural_id) {
            return;
        }
        if hir_in_external_macro(cx, expr.hir_id, expr.span) || is_from_proc_macro(cx, expr) {
            return;
        }
        // Neither macro guard covers a local `macro_rules!`, where the
        // suggestion would rewrite the *definition* with one call site's
        // text: measured turning a macro that removed environment
        // variables into one that adds arguments, and pasting a caller's
        // local where hygiene cannot resolve it. Two invocations would
        // also earn two suggestions at one span.
        if [expr.span, initial.span, shape.argument]
            .iter()
            .any(|span| span.from_expansion())
        {
            return;
        }
        emit(cx, expr, initial, folder, singular, plural_id, &shape);
    }
}

/// Report the fold, handing the fix over only where none of the reasons
/// to withhold it holds, and naming each reason that does.
fn emit<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    initial: &'tcx Expr<'tcx>,
    folder: &'tcx Expr<'tcx>,
    singular: DefId,
    plural_id: DefId,
    shape: &receiver::Shape,
) {
    let plural = cx.tcx.item_name(plural_id);
    let accumulator = cx.typeck_results().expr_ty(initial);
    let overridden = setter::overrides_the_plural(cx, plural_id, accumulator);
    let ambiguous_with = setter::another_trait_declaring(cx, plural_id, accumulator);
    // A *path* folder needs no import, so the plural may be out of
    // scope: measured as a machine-applicable `E0599` without this. The
    // import is compared with the resolved trait, not by name, since
    // another crate compiled as `command_extra` may lack the plural.
    let in_scope = imports(cx, expr, |imported| imported == cx.tcx.parent(plural_id));
    let typed = initial::fixes_its_own_type(cx, initial);
    let drops_a_type = folder::annotates_the_item(folder)
        || matches!(expr.kind, ExprKind::MethodCall(segment, ..) if segment.args.is_some());
    let drops_a_comment = drops_a_comment(cx, expr.span, [initial.span, shape.argument]);
    let mut applicability = if shape.reorderable
        && in_scope
        && !overridden
        && ambiguous_with.is_none()
        && typed
        && !drops_a_type
        && !drops_a_comment
    {
        Applicability::MachineApplicable
    } else {
        Applicability::Unspecified
    };
    let initial_text = if initial::holds_a_struct_literal(cx, initial) {
        format!(
            "({})",
            snippet_with_applicability(cx, initial.span, "..", &mut applicability),
        )
    } else {
        // The initial value becomes a method-call receiver, so one that
        // binds looser has to keep its own brackets: `*boxed` spliced raw
        // reads as `*boxed.plural(..)`, which derefs the *result*.
        Sugg::hir_with_applicability(cx, initial, "..", &mut applicability)
            .maybe_paren()
            .to_string()
    };
    let suggestion = format!(
        "{initial_text}.{plural}({}{})",
        shape.prefix,
        snippet_with_applicability(cx, shape.argument, "..", &mut applicability),
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
                diagnostic.help(
                    NEEDS_THE_IMPORT
                        .replace("{}", &trait_path(cx, folder, cx.tcx.parent(plural_id))),
                );
            }
            if overridden {
                diagnostic.help(OVERRIDDEN);
            }
            if !typed {
                diagnostic.help(UNTYPED);
            }
            if drops_a_type {
                diagnostic.help(DROPS_A_TYPE);
            }
            if drops_a_comment {
                diagnostic.help(DROPS_A_COMMENT);
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

/// Whether `fold` holds a comment outside every span in `kept`, which is
/// all the suggestion carries over. The spans in `kept` lie inside
/// `fold` and apart from each other, so comparing counts is enough.
fn drops_a_comment(cx: &LateContext<'_>, fold: Span, kept: [Span; 2]) -> bool {
    let count = |span| span_extract_comments(cx.tcx, span).len();
    count(fold) > kept.into_iter().map(count).sum::<usize>()
}

/// The trait's path as the folder spells it, or else its definition path.
///
/// The folder's spelling names the trait the way this crate does, where
/// the definition path names the crate as it was compiled: under a
/// manifest key that renames the dependency, as in
/// `ce = { package = "command-extra" }`, only the first resolves.
fn trait_path(cx: &LateContext<'_>, folder: &Expr<'_>, trait_id: DefId) -> String {
    if let ExprKind::Path(QPath::Resolved(_, path)) = folder.kind
        && let Some(first) = path
            .segments
            .iter()
            .find(|segment| segment.ident.name != kw::PathRoot)
        && let Some(last) = path
            .segments
            .iter()
            .find(|segment| segment.res == Res::Def(DefKind::Trait, trait_id))
        && let Some(text) = snippet_opt(cx, first.ident.span.to(last.ident.span))
    {
        return text;
    }
    // `crate::` before a local path, as rustc writes its own import
    // suggestions: a `use` of a bare item name does not resolve. The guard
    // sets that for as long as it lives.
    let _prefix = CratePrefixGuard::new();
    cx.tcx.def_path_str(trait_id)
}
