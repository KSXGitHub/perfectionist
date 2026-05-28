use std::sync::Mutex;

use rustc_ast::MacCall;
use rustc_ast::token::Delimiter;
use rustc_ast::tokenstream::TokenTree;
use rustc_lint::{EarlyContext, EarlyLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

use crate::common::{DefaultState, resolved_state};

mod config;
mod late;
mod purity;

use config::MacroArgumentBinding;
use late::MacroArgumentBindingLate;
use purity::{PurityContext, is_pure_expression, looks_like_expression, split_top_level_arguments};

declare_tool_lint! {
    /// ### What it does
    /// Flags impure expressions passed as top-level arguments to a
    /// function-like (`name!(...)`) or array-like (`name![...]`) macro
    /// invocation. The fix is to bind the expression to a `let` first
    /// and pass the binding instead, guaranteeing exactly-once
    /// evaluation.
    ///
    /// Curly-brace invocations (`name! { ... }`) are out of scope: by
    /// convention they are DSL bodies (`thread_local! { ... }`,
    /// `quote! { ... }`, `html! { ... }`) where the evaluation
    /// contract is the macro's, not the call site's.
    ///
    /// ### Why is this bad?
    /// A function-like or array-like macro may evaluate any top-level
    /// argument zero, one, or many times depending on its matcher.
    /// Functions guarantee exactly-once evaluation per argument; macros
    /// do not, even when the call shape looks identical. The classic
    /// case is `debug_assert_eq!`:
    ///
    /// ```rust,ignore
    /// debug_assert_eq!(map.insert(key, value), None, "duplicate");
    /// ```
    ///
    /// In debug builds the call runs and the assertion holds. In
    /// release builds `debug_assertions` is off, the body folds to
    /// `if false { ... }`, and the argument expressions are *not*
    /// evaluated — `insert` never runs and the map ends the function
    /// in a state the author did not intend. The bug only surfaces
    /// under `--release`.
    ///
    /// The same trap covers any macro that expands its capture more
    /// than once (`min!`/`max!`-style, retry loops): a side-effecting
    /// expression repeated produces wrong results.
    ///
    /// ### Terminology
    ///
    /// In this rule, **pure** means *safe for the surrounding macro
    /// to drop or duplicate*: evaluating the argument zero, one, or
    /// many times is observationally equivalent. **Impure** is
    /// anything else, and is what the rule flags.
    ///
    /// The classification is *syntactic*: the rule recognises a
    /// curated set of shapes known to satisfy the property and
    /// treats everything else as impure. A `const fn` call, a
    /// `Result::map` chain over a pure base, or `vec.fold(...)` is
    /// therefore impure under this rule unless its shape is
    /// recognised — the lint cannot prove side-effect-freedom in
    /// general, only spot it. The trade-off favours flagging
    /// side-effect-free expressions over silently passing a real
    /// hazard. The set is narrower than the functional-programming
    /// notion of purity and is keyed to what a macro can actually
    /// do with its captures, not to side-effect-freedom in the
    /// abstract.
    ///
    /// The recognised pure shapes are: literals, paths, field
    /// accesses, indexing of pure bases, dereferences, references,
    /// the logical / bitwise not of a pure expression (`!ready`),
    /// casts, the unit literal `()`, parenthesised / tuple /
    /// array-literal / array-repeat groups whose elements are all
    /// pure, binary chains of pure operands joined by
    /// side-effect-free operators, zero-arg method calls whose name
    /// is in the curated pure-getter set (`len`, `is_empty`,
    /// `as_str`, `as_bytes`, `as_ref`, `as_mut`, `as_deref`,
    /// `as_slice`, plus anything in `extra_pure_methods`), and
    /// calls to `core` / `std` macros whose expansion is a compile-
    /// time constant (`concat!`, `env!`, `option_env!`,
    /// `include_str!`, `include_bytes!`, `stringify!`, `cfg!`,
    /// `line!`, `column!`, `file!`, `module_path!`, plus anything in
    /// `extra_pure_macros`). A comparison like `vec.len() <= cap`
    /// evaluates the same way regardless of how many times the
    /// macro touches it, so binding it to a `let` would only force
    /// the comparison to run in release builds for no benefit; the
    /// same logic applies to `env!("HOME")` inside
    /// `debug_assert_eq!(...)` — there is nothing to evaluate at
    /// runtime.
    ///
    /// ### Example
    /// ```rust,ignore
    /// debug_assert_eq!(map.insert(key, value), None, "duplicate");
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// let ejected = map.insert(key, value);
    /// debug_assert_eq!(ejected, None, "duplicate");
    /// ```
    pub perfectionist::MACRO_ARGUMENT_BINDING,
    Warn,
    "macro invocation passes an impure expression that should be bound to a `let` first",
    report_in_external_macro: false
}

impl_lint_pass!(MacroArgumentBinding => [MACRO_ARGUMENT_BINDING]);
impl_lint_pass!(MacroArgumentBindingLate => [MACRO_ARGUMENT_BINDING]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[MACRO_ARGUMENT_BINDING]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("macro_argument_binding", DefaultState::Active) {
        return;
    }
    // Same split as `macro_trailing_comma`: a pre-expansion pass parks
    // violation spans, a late pass walks the HIR and emits each at the
    // deepest enclosing node so `cfg_attr`-wrapped `#[expect]` and
    // `#[allow]` attributes resolve correctly.
    lint_store.register_pre_expansion_pass(|| Box::new(MacroArgumentBinding::new()));
    lint_store.register_late_pass(|_| Box::new(MacroArgumentBindingLate));
}

/// Violation spans the pre-expansion pass has parked, waiting for the
/// late pass to anchor each at the deepest enclosing HIR node and
/// emit the diagnostic. `Span` is `Copy + Send + Sync` (a 32-bit id
/// into a session-side table), so a process-wide static is safe; the
/// `Mutex` just serialises the queue against parallel pre-expansion
/// passes within one compilation.
///
/// The static is private — child modules (`late`) read it through
/// Rust's standard descendant-reachability rule for non-`pub` items.
static PENDING_VIOLATIONS: Mutex<Vec<Span>> = Mutex::new(Vec::new());

impl EarlyLintPass for MacroArgumentBinding {
    fn check_mac(&mut self, _lint_context: &EarlyContext<'_>, mac_call: &MacCall) {
        // Curly-brace invocations are DSL bodies; skip them. The
        // delimiter check is on AST shape, not path / config, so it
        // lives here rather than behind `should_check_path`.
        if mac_call.args.delim == Delimiter::Brace {
            return;
        }
        if !self.should_check_path(&mac_call.path) {
            return;
        }
        let Some(arguments) = split_top_level_arguments(&mac_call.args.tokens) else {
            return;
        };
        let ctx = PurityContext {
            methods: self.pure_methods(),
            macros: self.pure_macros(),
        };
        for argument in arguments {
            check_argument(&argument, ctx);
        }
    }
}

fn check_argument(argument: &[TokenTree], ctx: PurityContext<'_>) {
    if argument.is_empty() {
        return;
    }
    if !looks_like_expression(argument) {
        return;
    }
    if is_pure_expression(argument, ctx) {
        return;
    }
    let first = argument.first().expect("non-empty checked above");
    let last = argument.last().expect("non-empty checked above");
    let span = first.span().to(last.span());
    queue(span);
}

fn queue(span: Span) {
    let mut guard = PENDING_VIOLATIONS
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    guard.push(span);
}
