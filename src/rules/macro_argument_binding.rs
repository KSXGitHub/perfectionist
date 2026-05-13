//! `perfectionist::macro_argument_binding` — flag non-trivial top-level
//! arguments to function-like (`name!(...)`) and array-like (`name![...]`)
//! macro invocations.
//!
//! Module layout:
//!
//! - [`config`] — user-facing configuration (`Mode`, `Config`,
//!   `MacroArgumentBinding` state) and the curated built-in deny /
//!   allow lists.
//! - [`triviality`] — top-level argument splitter, the
//!   `looks_like_expression` heuristic, and the trivial-expression
//!   token-stream walker.
//! - [`late`] — late pass that drains [`PENDING_VIOLATIONS`] and emits
//!   each diagnostic at the deepest enclosing HIR node.
//!
//! The pre-expansion pass below threads the three together: split
//! arguments, skip non-expression arguments, classify expressions,
//! park violation spans for the late pass to emit.

use std::collections::BTreeSet;
use std::sync::Mutex;

use rustc_ast::MacCall;
use rustc_ast::token::Delimiter;
use rustc_ast::tokenstream::TokenTree;
use rustc_lint::{EarlyContext, EarlyLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

mod config;
mod late;
mod triviality;

use config::MacroArgumentBinding;
use late::MacroArgumentBindingLate;
use triviality::{is_trivial_expression, looks_like_expression, split_top_level_arguments};

declare_tool_lint! {
    /// ### What it does
    /// Flags non-trivial expressions passed as top-level arguments to a
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
    /// Trivial arguments — literals, paths, field accesses, indexing
    /// of trivial bases, dereferences, references, casts, the unit
    /// literal `()`, parenthesised / tuple groups whose elements are
    /// all trivial, binary chains of trivial operands joined by
    /// side-effect-free operators, and zero-arg method calls whose
    /// name is in the curated pure-getter set (`len`, `is_empty`,
    /// `as_str`, `as_bytes`, `as_ref`, `as_mut`, `as_deref`,
    /// `as_slice`, plus anything in `extra_trivial_methods`) — are
    /// accepted as-is. A comparison like `vec.len() <= cap` evaluates
    /// the same way regardless of how many times the macro touches
    /// it, so binding it to a `let` would only force the comparison
    /// to run in release builds for no benefit. The lint focuses on
    /// arguments whose evaluation is itself observable.
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
    "macro invocation passes a non-trivial expression that should be bound to a `let` first",
    report_in_external_macro: false
}

impl_lint_pass!(MacroArgumentBinding => [MACRO_ARGUMENT_BINDING]);
impl_lint_pass!(MacroArgumentBindingLate => [MACRO_ARGUMENT_BINDING]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[MACRO_ARGUMENT_BINDING]);
}

pub fn register_pass(lint_store: &mut LintStore) {
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
        for argument in arguments {
            check_argument(&argument, self.trivial_methods());
        }
    }
}

fn check_argument(argument: &[TokenTree], trivial_methods: &BTreeSet<String>) {
    if argument.is_empty() {
        return;
    }
    if !looks_like_expression(argument) {
        return;
    }
    if is_trivial_expression(argument, trivial_methods) {
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
