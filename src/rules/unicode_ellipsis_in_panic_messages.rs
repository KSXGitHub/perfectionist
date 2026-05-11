use clippy_utils::diagnostics::span_lint_and_sugg;
use clippy_utils::macros::root_macro_call_first_node;
use clippy_utils::res::MaybeDef;
use rustc_ast::LitKind;
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind};
use rustc_lexer::{FrontmatterAllowed, LiteralKind, TokenKind, tokenize};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{BytePos, Pos, Span, Symbol, sym};

declare_tool_lint! {
    /// ### What it does
    /// Forbids U+2026 HORIZONTAL ELLIPSIS (`…`) in the message of a
    /// panic-family or assertion-style macro (`panic!`,
    /// `unimplemented!`, `todo!`, `unreachable!`, `assert!`,
    /// `assert_eq!`, `assert_ne!`, `debug_assert*!`) and in the
    /// `expect` / `expect_err` argument on `Option` and `Result`.
    /// Prefer the three-ASCII-dot form `...`.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// Panic and assertion messages surface in stderr, CI logs, crash
    /// reporters, and on terminals whose locale or encoding may not
    /// be UTF-8. ASCII `...` renders identically everywhere.
    ///
    /// ### Example
    /// ```rust,ignore
    /// panic!("could not parse manifest…");
    /// let manifest = load().expect("config missing…");
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// panic!("could not parse manifest...");
    /// let manifest = load().expect("config missing...");
    /// ```
    pub perfectionist::UNICODE_ELLIPSIS_IN_PANIC_MESSAGES,
    Warn,
    "U+2026 HORIZONTAL ELLIPSIS in panic / assertion / expect messages; prefer `...`",
    // Load-bearing: the user-supplied literal inside `panic!`,
    // `assert!`, `assert_eq!`, etc. lives inside a `core` macro
    // expansion. With the default `false` rustc would treat every
    // diagnostic on those literals as "in an external macro" and
    // drop it before reaching the user.
    report_in_external_macro: true
}

const CONFIG_KEY: &str = "perfectionist::unicode_ellipsis_in_panic_messages";

const DEFAULT_MACROS: &[&str] = &[
    "panic",
    "unimplemented",
    "todo",
    "unreachable",
    "debug_unreachable",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
];

const DEFAULT_METHODS: &[&str] = &["expect", "expect_err"];

#[derive(Debug, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {
    macros: Vec<String>,
    methods: Vec<String>,
    also_flag: Vec<char>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            macros: DEFAULT_MACROS
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            methods: DEFAULT_METHODS
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            also_flag: Vec::new(),
        }
    }
}

pub struct UnicodeEllipsisInPanicMessages {
    flagged_chars: Vec<char>,
    macros: Vec<Symbol>,
    methods: Vec<Symbol>,
}

impl UnicodeEllipsisInPanicMessages {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let mut flagged_chars = vec!['\u{2026}'];
        for character in config.also_flag {
            if !flagged_chars.contains(&character) {
                flagged_chars.push(character);
            }
        }
        Self {
            flagged_chars,
            macros: config
                .macros
                .iter()
                .map(|name| Symbol::intern(name))
                .collect(),
            methods: config
                .methods
                .iter()
                .map(|name| Symbol::intern(name))
                .collect(),
        }
    }
}

impl_lint_pass!(UnicodeEllipsisInPanicMessages => [UNICODE_ELLIPSIS_IN_PANIC_MESSAGES]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[UNICODE_ELLIPSIS_IN_PANIC_MESSAGES]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    lint_store.register_late_pass(|_| Box::new(UnicodeEllipsisInPanicMessages::new()));
}

impl<'tcx> LateLintPass<'tcx> for UnicodeEllipsisInPanicMessages {
    fn check_expr(&mut self, lint_context: &LateContext<'tcx>, expr: &Expr<'tcx>) {
        // Panic / assertion macros: `root_macro_call_first_node`
        // returns `Some` exactly when `expr` is the boundary HIR
        // node of the outermost macro expansion, so each call's
        // source is scanned once.
        if let Some(macro_call) = root_macro_call_first_node(lint_context, expr) {
            let macro_name = lint_context.tcx.item_name(macro_call.def_id);
            if self.macros.contains(&macro_name) {
                self.scan_macro_call_source(lint_context, macro_call.span, macro_name);
            }
        }
        // `expect` / `expect_err` on `Option` / `Result`.
        if let ExprKind::MethodCall(path_segment, receiver, arguments, _) = expr.kind
            && self.methods.contains(&path_segment.ident.name)
            && receiver_is_option_or_result(lint_context, receiver)
            && let Some(message_argument) = arguments.first()
            && let ExprKind::Lit(literal) = message_argument.kind
            && matches!(literal.node, LitKind::Str(..))
        {
            let context = format!("`{}` message", path_segment.ident.name);
            if let Ok(snippet) = lint_context
                .sess()
                .source_map()
                .span_to_snippet(literal.span)
            {
                self.scan_literal(lint_context, literal.span, &snippet, &context);
            }
        }
    }
}

fn receiver_is_option_or_result<'tcx>(
    lint_context: &LateContext<'tcx>,
    receiver: &Expr<'tcx>,
) -> bool {
    let receiver_type = lint_context.typeck_results().expr_ty(receiver).peel_refs();
    receiver_type.is_diag_item(lint_context, sym::Option)
        || receiver_type.is_diag_item(lint_context, sym::Result)
}

impl UnicodeEllipsisInPanicMessages {
    fn scan_macro_call_source(
        &self,
        lint_context: &LateContext<'_>,
        call_span: Span,
        macro_name: Symbol,
    ) {
        let Ok(snippet) = lint_context.sess().source_map().span_to_snippet(call_span) else {
            return;
        };
        let context = format!("`{macro_name}!` message");
        // Track delimiter nesting so we only scan literals at the
        // macro's own argument level. The snippet starts with
        // `macro_name!(`/`[`/`{`, which opens depth 1; literals
        // belonging to the panic message live at exactly depth 1.
        // Anything deeper is an argument of a nested call (e.g.,
        // `format!("...")` or `include_str!("path")`) whose literal
        // is not the panic message.
        //
        // Within depth 1 we also count commas to skip the non-message
        // arguments of `assert!`-family macros: `assert!(cond, msg)`
        // skips one comma-separated argument before the message,
        // `assert_eq!(a, b, msg)` and `assert_ne!`/`debug_assert_eq!`/
        // `debug_assert_ne!` skip two. Scanning value-position
        // literals would otherwise rewrite comparison operands.
        let skip_arguments = arguments_before_message(macro_name);
        let mut byte_offset: u32 = 0;
        let mut depth: u32 = 0;
        let mut top_level_comma_count: u32 = 0;
        for token in tokenize(&snippet, FrontmatterAllowed::No) {
            let token_length = token.len;
            match token.kind {
                TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::OpenBrace => {
                    depth = depth.saturating_add(1);
                }
                TokenKind::CloseParen | TokenKind::CloseBracket | TokenKind::CloseBrace => {
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Comma if depth == 1 => {
                    top_level_comma_count = top_level_comma_count.saturating_add(1);
                }
                TokenKind::Literal { kind, .. }
                    if depth == 1
                        && top_level_comma_count >= skip_arguments
                        && is_display_string_literal(kind) =>
                {
                    let token_start = byte_offset as usize;
                    let token_end = token_start + token_length as usize;
                    let literal_snippet = &snippet[token_start..token_end];
                    let token_lo = call_span.lo() + BytePos::from_u32(byte_offset);
                    let token_hi = token_lo + BytePos::from_u32(token_length);
                    let token_span =
                        Span::new(token_lo, token_hi, call_span.ctxt(), call_span.parent());
                    self.scan_literal(lint_context, token_span, literal_snippet, &context);
                }
                _ => {}
            }
            byte_offset = byte_offset
                .checked_add(token_length)
                .expect("snippet offset overflowed u32");
        }
    }

    fn scan_literal(
        &self,
        lint_context: &LateContext<'_>,
        literal_span: Span,
        literal_snippet: &str,
        context: &str,
    ) {
        let Some((prefix_length, suffix_length)) = string_literal_quote_lengths(literal_snippet)
        else {
            return;
        };
        let body = &literal_snippet[prefix_length..literal_snippet.len() - suffix_length];
        for (byte_offset, character) in body.char_indices() {
            if !self.flagged_chars.contains(&character) {
                continue;
            }
            let character_length = character.len_utf8() as u32;
            let span_start =
                literal_span.lo() + BytePos::from_u32((prefix_length + byte_offset) as u32);
            let span_end = span_start + BytePos::from_u32(character_length);
            let span = Span::new(
                span_start,
                span_end,
                literal_span.ctxt(),
                literal_span.parent(),
            );
            let applicability = if character == '\u{2026}' {
                Applicability::MachineApplicable
            } else {
                Applicability::MaybeIncorrect
            };
            span_lint_and_sugg(
                lint_context,
                UNICODE_ELLIPSIS_IN_PANIC_MESSAGES,
                span,
                format!(
                    "Unicode `{character}` (U+{:04X}) in {context}",
                    character as u32
                ),
                "use ASCII `...` instead",
                "...".to_owned(),
                applicability,
            );
        }
    }
}

fn is_display_string_literal(kind: LiteralKind) -> bool {
    matches!(kind, LiteralKind::Str { .. } | LiteralKind::RawStr { .. })
}

/// How many comma-separated top-level arguments precede the message
/// argument for a given macro. `0` for macros whose first argument is
/// itself the message (`panic!`, `todo!`, …); `1` for `assert!` /
/// `debug_assert!` (the condition comes first); `2` for `assert_eq!`
/// / `assert_ne!` / `debug_assert_eq!` / `debug_assert_ne!` (the two
/// values come first).
///
/// Unknown macros — including any added through the configuration's
/// `macros` knob — default to `0`. A project that adds an
/// assertion-shaped custom macro through configuration accepts the
/// false positive in that case; correctly handling it would require
/// a per-macro skip-count configuration knob that the planning file
/// did not call for.
fn arguments_before_message(macro_name: Symbol) -> u32 {
    match macro_name.as_str() {
        "assert" | "debug_assert" => 1,
        "assert_eq" | "assert_ne" | "debug_assert_eq" | "debug_assert_ne" => 2,
        _ => 0,
    }
}

/// Return `(prefix_length, suffix_length)` covering the opening and
/// closing delimiters of a Rust string-literal snippet, or `None` if
/// the snippet does not look like a string literal whose body we can
/// scan as plain text.
///
/// Recognises plain (`"..."`) and raw (`r"..."`, `r#"..."#`, …)
/// strings. Byte / C-string forms are excluded — the lint operates on
/// display strings.
fn string_literal_quote_lengths(snippet: &str) -> Option<(usize, usize)> {
    let bytes = snippet.as_bytes();
    let mut index = 0;
    let mut hash_count = 0;
    if index < bytes.len() && bytes[index] == b'r' {
        index += 1;
        while index < bytes.len() && bytes[index] == b'#' {
            hash_count += 1;
            index += 1;
        }
    }
    if index >= bytes.len() || bytes[index] != b'"' {
        return None;
    }
    let prefix_length = index + 1;
    let expected_suffix_length = hash_count + 1;
    if bytes.len() < prefix_length + expected_suffix_length {
        return None;
    }
    let suffix_start = bytes.len() - expected_suffix_length;
    if bytes[suffix_start] != b'"' {
        return None;
    }
    for trailing_hash_index in 0..hash_count {
        if bytes[suffix_start + 1 + trailing_hash_index] != b'#' {
            return None;
        }
    }
    Some((prefix_length, expected_suffix_length))
}
