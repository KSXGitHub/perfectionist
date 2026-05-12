use std::collections::BTreeSet;
use std::sync::Mutex;

use crate::macro_path::{matches_any, parse_path};
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_ast::MacCall;
use rustc_ast::token::{Delimiter, IdentIsRaw, TokenKind};
use rustc_ast::tokenstream::{TokenStream, TokenTree};
use rustc_hir as hir;
use rustc_hir::intravisit::{self, Visitor};
use rustc_lint::{EarlyContext, EarlyLintPass, LateContext, LateLintPass, LintStore};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, kw};

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

const CONFIG_KEY: &str = "perfectionist::macro_argument_binding";

/// Macros whose argument list is checked unconditionally because the
/// expansion is known to evaluate captures conditionally on a `cfg`
/// (`debug_assert*`) or to drop them entirely in release builds.
const BUILTIN_DENY: &[&str] = &["debug_assert", "debug_assert_eq", "debug_assert_ne"];

/// Macros known to evaluate every top-level argument exactly once. The
/// list mirrors the curated set in `macro_trailing_comma`, with the
/// conditional-evaluation families (`log::*`, `tracing::*`) removed
/// because those *do* drop arguments below the configured filter level.
const BUILTIN_ALLOW: &[&str] = &[
    "format",
    "format_args",
    "print",
    "println",
    "eprint",
    "eprintln",
    "write",
    "writeln",
    "vec",
    "panic",
    "unimplemented",
    "todo",
    "unreachable",
    "assert",
    "assert_eq",
    "assert_ne",
    "matches",
    "dbg",
    "anyhow",
];

/// Eligibility mode. The default is `AllowAndDeny`. The matcher-based
/// mode described in `planned-rules/macro-argument-binding.md` is not
/// yet implemented and is therefore not exposed as a value here; a
/// `dylint.toml` that names it will fail to deserialise with a
/// useful error.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    /// Flag only invocations of the curated deny list (`debug_assert*`
    /// plus `deny_extra`). Every other macro is silently accepted.
    DenyOnly,
    /// Flag every function-like or array-like invocation that carries
    /// a non-trivial top-level argument, regardless of any built-in
    /// classification — unless the invocation matches an `allow_extra`
    /// entry. The built-in allow list is deliberately ignored in this
    /// mode; project exceptions go in `allow_extra`.
    Blanket,
    /// Curated deny list plus curated allow list, both extensible via
    /// `deny_extra` / `allow_extra`. Macros on neither list are
    /// flagged — flagging unrecognised macros is deliberate so the
    /// rule remains useful in projects that depend on uncatalogued
    /// proc macros.
    #[default]
    AllowAndDeny,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {
    /// Master on/off switch for the rule. Defaults to `true`. Set
    /// to `false` to silence every diagnostic this lint would emit
    /// without having to enumerate every macro under `ignore`.
    enabled: bool,
    /// Eligibility mode. See [`Mode`].
    mode: Mode,
    /// Macros added to the built-in deny list. Each entry is a
    /// fully-qualified macro path (no trailing `!`) or a bare macro
    /// name to match by final segment only.
    deny_extra: Vec<String>,
    /// Macros added to the built-in allow list. Same matching rules
    /// as `deny_extra`. Only meaningful in `AllowAndDeny` and
    /// `Blanket` modes; in `DenyOnly` the allow list is unused.
    allow_extra: Vec<String>,
    /// Macros to skip entirely, regardless of which list they would
    /// otherwise hit. Same matching rules as `deny_extra`.
    ignore: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: Mode::default(),
            deny_extra: Vec::new(),
            allow_extra: Vec::new(),
            ignore: Vec::new(),
        }
    }
}

pub struct MacroArgumentBinding {
    enabled: bool,
    mode: Mode,
    /// Built-in deny list plus `deny_extra`. Used in `DenyOnly` and
    /// `AllowAndDeny`.
    deny: BTreeSet<Vec<String>>,
    /// Built-in allow list plus `allow_extra`. Used only in
    /// `AllowAndDeny`; `Blanket` deliberately ignores the built-in
    /// allow list and consults `allow_extra` alone.
    allow: BTreeSet<Vec<String>>,
    /// Only the user-supplied `allow_extra` entries. Used in
    /// `Blanket` mode, which has no built-in allow list per the rule
    /// docs (`planned-rules/macro-argument-binding.md`).
    allow_extra: BTreeSet<Vec<String>>,
    ignore: BTreeSet<Vec<String>>,
}

impl MacroArgumentBinding {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let extra_deny = parse_path_list(&config.deny_extra);
        let extra_allow = parse_path_list(&config.allow_extra);
        let deny = merge_with_builtins(BUILTIN_DENY, &extra_deny);
        let allow = merge_with_builtins(BUILTIN_ALLOW, &extra_allow);
        let ignore = parse_path_list(&config.ignore);
        Self {
            enabled: config.enabled,
            mode: config.mode,
            deny,
            allow,
            allow_extra: extra_allow,
            ignore,
        }
    }

    fn arguments_should_be_checked(&self, mac_call: &MacCall) -> bool {
        let on_deny = matches_any(&mac_call.path, &self.deny);
        match self.mode {
            Mode::DenyOnly => on_deny,
            Mode::Blanket => !matches_any(&mac_call.path, &self.allow_extra),
            Mode::AllowAndDeny => on_deny || !matches_any(&mac_call.path, &self.allow),
        }
    }
}

fn parse_path_list(raw_entries: &[String]) -> BTreeSet<Vec<String>> {
    raw_entries
        .iter()
        .map(|entry| parse_path(entry))
        .filter(|parsed| !parsed.is_empty())
        .collect()
}

fn merge_with_builtins(builtin: &[&str], extras: &BTreeSet<Vec<String>>) -> BTreeSet<Vec<String>> {
    let mut set: BTreeSet<Vec<String>> = builtin
        .iter()
        .map(|name| vec![(*name).to_owned()])
        .collect();
    set.extend(extras.iter().cloned());
    set
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

static PENDING_VIOLATIONS: Mutex<Vec<Span>> = Mutex::new(Vec::new());

pub struct MacroArgumentBindingLate;

impl EarlyLintPass for MacroArgumentBinding {
    fn check_mac(&mut self, _lint_context: &EarlyContext<'_>, mac_call: &MacCall) {
        if !self.enabled {
            return;
        }
        let args = &mac_call.args;
        // Curly-brace invocations are DSL bodies; skip them.
        if args.delim == Delimiter::Brace {
            return;
        }
        if matches_any(&mac_call.path, &self.ignore) {
            return;
        }
        if !self.arguments_should_be_checked(mac_call) {
            return;
        }
        let Some(arguments) = split_top_level_arguments(&args.tokens) else {
            return;
        };
        for argument in arguments {
            check_argument(&argument);
        }
    }
}

fn check_argument(argument: &[TokenTree]) {
    if argument.is_empty() {
        return;
    }
    if !looks_like_expression(argument) {
        return;
    }
    if is_trivial_expression(argument) {
        return;
    }
    let first = argument.first().expect("non-empty checked above");
    let last = argument.last().expect("non-empty checked above");
    let span = first.span().to(last.span());
    queue(span);
}

/// Heuristic: does the argument plausibly parse as a single Rust
/// expression? The rule docs say "skip arguments that don't parse as a
/// single expression (`name: type`, `name = value`, etc. are syntactic
/// positions the macro author chose)" and prescribe a `Parser::parse_expr`
/// re-parse to make that call. We approximate without `rustc_parse` to
/// avoid emitting parser-recovery diagnostics for arbitrary macro
/// inputs: a top-level `=>` token is a match-arm separator (`matches!`,
/// `impl_lint_pass!`-style `Type => [LINT_NAMES]` DSLs) and is never
/// part of a single Rust expression. Other non-expression markers like
/// `name: type` and `name = value` are not reliably distinguishable
/// from valid expression syntax (`expr: type` ascription, assignment),
/// and a future re-parse-based implementation will subsume this check.
fn looks_like_expression(argument: &[TokenTree]) -> bool {
    !argument.iter().any(|tree| {
        matches!(
            tree,
            TokenTree::Token(token, _) if token.kind == TokenKind::FatArrow,
        )
    })
}

fn queue(span: Span) {
    let mut guard = PENDING_VIOLATIONS
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    guard.push(span);
}

/// Split the top-level token stream of a macro invocation into one
/// segment per comma-separated argument. Returns `None` if a top-level
/// `;` is encountered (the repeat form, `vec![v; count]`), which
/// signals that the invocation is not a comma-separated argument list
/// and the rule skips the whole call.
///
/// `=>` is ordinary content here — match-arm syntax inside `matches!`
/// shows up as a top-level fat arrow but is meaningful to the macro,
/// not a separator. The walker passes it through unchanged so each
/// argument's `looks_like_expression` check can skip it as a non-
/// expression position the macro author chose.
fn split_top_level_arguments(stream: &TokenStream) -> Option<Vec<Vec<TokenTree>>> {
    let mut arguments: Vec<Vec<TokenTree>> = Vec::new();
    let mut current: Vec<TokenTree> = Vec::new();
    for tree in stream.iter() {
        if let TokenTree::Token(token, _) = tree {
            match token.kind {
                TokenKind::Semi => return None,
                TokenKind::Comma => {
                    arguments.push(std::mem::take(&mut current));
                    continue;
                }
                _ => {}
            }
        }
        current.push(tree.clone());
    }
    if !current.is_empty() {
        arguments.push(current);
    }
    Some(arguments)
}

/// Returns `true` if the entire token slice forms a "trivial"
/// expression per the rule's grammar. Triviality is purely syntactic:
/// the seven shapes the rule docs enumerate, recursive on operands.
/// Anything outside that grammar is non-trivial — including `const fn`
/// calls and other "morally pure" expressions.
fn is_trivial_expression(tokens: &[TokenTree]) -> bool {
    take_trivial_expression(tokens).is_some_and(<[_]>::is_empty)
}

fn take_trivial_expression(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let after_atom = take_trivial_atom(tokens)?;
    Some(take_trivial_suffixes(after_atom))
}

fn take_trivial_atom(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let (head, rest) = tokens.split_first()?;
    let TokenTree::Token(token, _) = head else {
        return None;
    };
    match token.kind {
        TokenKind::Literal(_) => Some(rest),
        // `true` and `false` are keyword idents, not `Literal` tokens.
        TokenKind::Ident(name, IdentIsRaw::No) if name == kw::True || name == kw::False => {
            Some(rest)
        }
        // `&` expr or `&mut` expr.
        TokenKind::And => take_reference_tail(rest),
        // `&&` expr or `&& mut` expr (double reference).
        TokenKind::AndAnd => take_reference_tail(rest),
        // `*expr` (deref).
        TokenKind::Star => take_trivial_expression(rest),
        // Path: ident (`::` ident)*.
        TokenKind::Ident(_, _) => Some(take_path_tail(rest)),
        // Leading `::` — must be followed by an ident.
        TokenKind::PathSep => take_path_after_sep(rest),
        _ => None,
    }
}

fn take_reference_tail(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let after_mut = match tokens.split_first() {
        Some((TokenTree::Token(token, _), rest)) if token.is_keyword(kw::Mut) => rest,
        _ => tokens,
    };
    take_trivial_expression(after_mut)
}

fn take_path_tail(mut tokens: &[TokenTree]) -> &[TokenTree] {
    while let Some((TokenTree::Token(sep, _), after_sep)) = tokens.split_first() {
        if sep.kind != TokenKind::PathSep {
            break;
        }
        let Some((TokenTree::Token(ident, _), after_ident)) = after_sep.split_first() else {
            break;
        };
        if !matches!(ident.kind, TokenKind::Ident(_, _)) {
            break;
        }
        tokens = after_ident;
    }
    tokens
}

fn take_path_after_sep(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let (ident, rest) = tokens.split_first()?;
    let TokenTree::Token(token, _) = ident else {
        return None;
    };
    if !matches!(token.kind, TokenKind::Ident(_, _)) {
        return None;
    }
    Some(take_path_tail(rest))
}

fn take_trivial_suffixes(mut tokens: &[TokenTree]) -> &[TokenTree] {
    loop {
        let Some((head, rest)) = tokens.split_first() else {
            return tokens;
        };
        match head {
            TokenTree::Token(token, _) => match token.kind {
                // `.ident` (field access) or `.0` (tuple index).
                TokenKind::Dot => {
                    let Some((next, after)) = rest.split_first() else {
                        return tokens;
                    };
                    let TokenTree::Token(next_token, _) = next else {
                        return tokens;
                    };
                    match next_token.kind {
                        TokenKind::Ident(_, _) | TokenKind::Literal(_) => tokens = after,
                        _ => return tokens,
                    }
                }
                // `as path` — type annotation. Only path-shaped types
                // are recognised; references, slices, function pointers,
                // etc. fall back to non-trivial.
                TokenKind::Ident(name, IdentIsRaw::No) if name == kw::As => {
                    let Some(after) = take_trivial_type(rest) else {
                        return tokens;
                    };
                    tokens = after;
                }
                _ => return tokens,
            },
            // `[expr]` — index. Both base and index must be trivial;
            // the recursion happens here for the index.
            TokenTree::Delimited(_, _, Delimiter::Bracket, inner) => {
                if !is_trivial_expression_stream(inner) {
                    return tokens;
                }
                tokens = rest;
            }
            _ => return tokens,
        }
    }
}

fn take_trivial_type(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let (head, rest) = tokens.split_first()?;
    let TokenTree::Token(token, _) = head else {
        return None;
    };
    match token.kind {
        TokenKind::Ident(_, _) => Some(take_path_tail(rest)),
        TokenKind::PathSep => take_path_after_sep(rest),
        _ => None,
    }
}

fn is_trivial_expression_stream(stream: &TokenStream) -> bool {
    let trees: Vec<TokenTree> = stream.iter().cloned().collect();
    is_trivial_expression(&trees)
}

impl<'tcx> LateLintPass<'tcx> for MacroArgumentBindingLate {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let pending: Vec<Span> = {
            let mut guard = PENDING_VIOLATIONS
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            std::mem::take(&mut *guard)
        };
        if pending.is_empty() {
            return;
        }
        let tcx = lint_context.tcx;
        let mut best: Vec<hir::HirId> = vec![hir::CRATE_HIR_ID; pending.len()];
        let mut finder = EnclosingHirFinder {
            tcx,
            pending: &pending,
            best: &mut best,
        };
        tcx.hir_walk_toplevel_module(&mut finder);
        for (&span, &hir_id) in pending.iter().zip(best.iter()) {
            emit(lint_context, hir_id, span);
        }
    }
}

fn emit(lint_context: &LateContext<'_>, hir_id: hir::HirId, span: Span) {
    span_lint_hir_and_then(
        lint_context,
        MACRO_ARGUMENT_BINDING,
        hir_id,
        span,
        "non-trivial expression passed directly to a macro",
        |diag| {
            diag.help(
                "bind the expression to a `let` immediately before the macro \
                 call so it is evaluated exactly once, regardless of how the \
                 macro expands",
            );
        },
    );
}

/// Walk the HIR once and, for each pending violation span, record the
/// deepest HIR node whose span contains it. Mirrors the equivalent
/// finder in `macro_trailing_comma`; the two cannot share a single
/// instance cheaply because each rule's pending list is a different
/// type, but the walk shape is identical.
struct EnclosingHirFinder<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pending: &'a [Span],
    best: &'a mut [hir::HirId],
}

impl<'a, 'tcx> EnclosingHirFinder<'a, 'tcx> {
    fn update(&mut self, hir_id: hir::HirId, span: Span) {
        for (index, &target) in self.pending.iter().enumerate() {
            if !span.contains(target) {
                continue;
            }
            self.best[index] = hir_id;
        }
    }
}

impl<'tcx> Visitor<'tcx> for EnclosingHirFinder<'_, 'tcx> {
    type NestedFilter = nested_filter::All;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) {
        self.update(item.hir_id(), item.span);
        intravisit::walk_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'tcx hir::TraitItem<'tcx>) {
        self.update(item.hir_id(), item.span);
        intravisit::walk_trait_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'tcx hir::ImplItem<'tcx>) {
        self.update(item.hir_id(), item.span);
        intravisit::walk_impl_item(self, item);
    }

    fn visit_foreign_item(&mut self, item: &'tcx hir::ForeignItem<'tcx>) {
        self.update(item.hir_id(), item.span);
        intravisit::walk_foreign_item(self, item);
    }

    fn visit_block(&mut self, block: &'tcx hir::Block<'tcx>) {
        self.update(block.hir_id, block.span);
        intravisit::walk_block(self, block);
    }

    fn visit_stmt(&mut self, stmt: &'tcx hir::Stmt<'tcx>) {
        self.update(stmt.hir_id, stmt.span);
        intravisit::walk_stmt(self, stmt);
    }

    fn visit_local(&mut self, local: &'tcx hir::LetStmt<'tcx>) {
        self.update(local.hir_id, local.span);
        intravisit::walk_local(self, local);
    }

    fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        self.update(expr.hir_id, expr.span);
        intravisit::walk_expr(self, expr);
    }

    fn visit_pat(&mut self, pat: &'tcx hir::Pat<'tcx>) {
        self.update(pat.hir_id, pat.span);
        intravisit::walk_pat(self, pat);
    }
}
