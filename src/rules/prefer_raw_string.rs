use clippy_utils::diagnostics::span_lint_and_sugg;
use rustc_ast::{LitKind, StrStyle};
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

declare_tool_lint! {
    /// ### What it does
    /// Forbids regular string literals whose only backslash escapes
    /// are ones a raw string would express verbatim — `\"`, `\\`,
    /// and `\'`. The autofix rewrites the literal to the raw form
    /// `r"..."` / `r#"..."#`, picking the smallest hash count that
    /// avoids a delimiter collision.
    ///
    /// This includes literals passed as arguments to macros such as
    /// `println!`, `format!`, `vec!`, and `assert!`. Suppress per
    /// call site with `#[allow(perfectionist::prefer_raw_string)]`
    /// when the regular form is deliberately preferred.
    ///
    /// Pattern-position literals (e.g. `match s { "C:\\path" => ... }`)
    /// are out of scope — the rule only visits expression literals.
    ///
    /// Whitespace and control-character escapes (`\n`, `\t`, `\r`,
    /// `\0`) and Unicode escapes (`\x..`, `\u{..}`) are exempt — a
    /// raw string cannot express them, and the regular form is the
    /// only choice. A literal that mixes eliminable and
    /// inexpressible escapes is also left alone; the rewrite would
    /// force the author to split the literal or fall back to
    /// `concat!`, which loses more than it gains.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. The
    /// rule trades one noise source (interior backslash escapes)
    /// for a slightly more elaborate string syntax. The benefit is
    /// highest in strings full of file paths, regex patterns, JSON
    /// snippets, or embedded source code — all of which would
    /// otherwise be a sea of `\\` and `\"`.
    ///
    /// ### Example
    /// ```rust,ignore
    /// let json = "{\"name\":\"foo\"}";
    /// let path = "C:\\Users\\foo\\bar";
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// let json = r#"{"name":"foo"}"#;
    /// let path = r"C:\Users\foo\bar";
    /// ```
    pub perfectionist::PREFER_RAW_STRING,
    Warn,
    "string literal contains only raw-expressible escapes; prefer the raw-string form",
    // Load-bearing: an escaped string literal passed as a `println!` /
    // `format!` / `vec!` / etc. argument lives inside a `core` macro
    // expansion. With the default `false` rustc would treat every
    // diagnostic on those literals as "in an external macro" and
    // drop it before reaching the user, even though the literal
    // itself is user-written. The `span_to_snippet` guard in
    // `check_expr` already bails on synthesised spans, so
    // compiler-generated literals stay safely out of scope.
    report_in_external_macro: true
}

const CONFIG_KEY: &str = "perfectionist::prefer_raw_string";

/// Default eligible escape sequences: the three escapes that a raw
/// string can express verbatim with no escape at all.
const DEFAULT_ESCAPES_ELIGIBLE: &[&str] = &[r#"\""#, r"\\", r"\'"];

#[derive(Debug, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {
    /// Master on/off switch for the rule. Set to `false` to silence
    /// every diagnostic without enumerating individual literals.
    enabled: bool,
    /// Minimum number of eliminable escapes a string must contain
    /// before the lint fires. Default 1 catches every escapable
    /// string; set to 2 to skip single-escape literals where the
    /// raw form is arguably noisier than the original.
    min_escapes_to_trigger: usize,
    /// Escape sequences considered eliminable by switching to raw
    /// form. Only the three Rust escapes whose decoded character
    /// is exactly the byte after the backslash — `"\""`, `"\\"`,
    /// `"\\'"` — are accepted; entries listed here that fall
    /// outside that closed set are silently dropped. (`\n`, `\t`,
    /// `\xNN`, `\u{...}` and other escapes decode to a different
    /// character and cannot be expressed verbatim in a raw string,
    /// so they have no place in this list.) Use this knob to
    /// narrow eligibility — e.g. `["\\\""]` to only flag literals
    /// whose sole escapes are escaped quotes — not to extend it.
    escapes_eligible: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            min_escapes_to_trigger: 1,
            escapes_eligible: DEFAULT_ESCAPES_ELIGIBLE
                .iter()
                .map(|entry| (*entry).to_owned())
                .collect(),
        }
    }
}

pub struct PreferRawString {
    enabled: bool,
    min_escapes_to_trigger: usize,
    escapes_eligible: Vec<String>,
}

impl PreferRawString {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        // Drop entries that aren't one of the three self-decoding
        // escapes (`\"`, `\\`, `\'`). Anything else — `\n`, `\t`,
        // `\xNN`, `\u{...}`, ill-formed shapes — would break
        // `eliminable_decoded`'s "second char is the decoded form"
        // contract and let the `MachineApplicable` autofix silently
        // corrupt user code. Filter rather than reject so a stray
        // entry in the config table doesn't take the whole rule
        // offline.
        let escapes_eligible = config
            .escapes_eligible
            .into_iter()
            .filter(|entry| is_supported_eligible_entry(entry))
            .collect();
        Self {
            enabled: config.enabled,
            min_escapes_to_trigger: config.min_escapes_to_trigger,
            escapes_eligible,
        }
    }
}

impl_lint_pass!(PreferRawString => [PREFER_RAW_STRING]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[PREFER_RAW_STRING]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    lint_store.register_late_pass(|_| Box::new(PreferRawString::new()));
}

impl<'tcx> LateLintPass<'tcx> for PreferRawString {
    fn check_expr(&mut self, lint_context: &LateContext<'tcx>, expr: &Expr<'tcx>) {
        if !self.enabled {
            return;
        }
        let ExprKind::Lit(literal) = expr.kind else {
            return;
        };
        if !matches!(literal.node, LitKind::Str(_, StrStyle::Cooked)) {
            return;
        }
        let Ok(snippet) = lint_context
            .sess()
            .source_map()
            .span_to_snippet(literal.span)
        else {
            return;
        };
        // Belt-and-braces: defend against any source spelling that
        // doesn't actually look like a cooked string literal at the
        // syntactic level (synthesised spans, edge cases). The
        // `Cooked` check above already covers the normal path.
        let Some(body) = snippet
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
        else {
            return;
        };
        let Some(scan) = scan_body(body, &self.escapes_eligible) else {
            return;
        };
        // The `eliminable_count == 0` guard is stricter than the
        // planning file's `count >= threshold` rule and deliberate:
        // suggesting `r"hello"` for `"hello"` would just trip
        // `clippy::needless_raw_strings` on the next pass. The
        // guard kicks in if a user sets `min_escapes_to_trigger`
        // to 0, which the planning file doesn't expect but the
        // schema doesn't forbid.
        if scan.eliminable_count == 0 || scan.eliminable_count < self.min_escapes_to_trigger {
            return;
        }
        let n_hashes = minimal_hash_count(&scan.decoded);
        let hashes = "#".repeat(n_hashes);
        let suggestion = format!("r{hashes}\"{}\"{hashes}", scan.decoded);
        span_lint_and_sugg(
            lint_context,
            PREFER_RAW_STRING,
            literal.span,
            "string literal uses escapes that a raw string would avoid",
            "use a raw string",
            suggestion,
            Applicability::MachineApplicable,
        );
    }
}

struct ScanResult {
    eliminable_count: usize,
    decoded: String,
}

/// Walk the body of a cooked string literal (everything between the
/// surrounding quotes) and classify each escape. Returns `None` if
/// the body contains any non-raw escape — `\n`, `\t`, `\r`, `\0`,
/// `\xNN`, `\u{...}`, line continuations, or any other backslash
/// sequence that is not listed in the configured `escapes_eligible`.
fn scan_body(body: &str, eligible: &[String]) -> Option<ScanResult> {
    let mut rest = body;
    let mut eliminable_count: usize = 0;
    let mut decoded = String::with_capacity(body.len());
    while !rest.is_empty() {
        if let Some((escape, remainder)) = take_escape_eliminable(rest, eligible) {
            decoded.push_str(eliminable_decoded(escape));
            eliminable_count = eliminable_count.saturating_add(1);
            rest = remainder;
            continue;
        }
        if take_escape_non_raw(rest).is_some() {
            return None;
        }
        let (literal, remainder) = take_literal_char(rest)?;
        decoded.push_str(literal);
        rest = remainder;
    }
    Some(ScanResult {
        eliminable_count,
        decoded,
    })
}

/// Take a prefix of `input` that matches one of the configured
/// eligible escape sequences. Each entry is matched literally
/// against the input — no decoding, no normalisation. Entries
/// reach this function only after [`is_supported_eligible_entry`]
/// has accepted them, so they are non-empty by construction.
fn take_escape_eliminable<'a>(input: &'a str, eligible: &[String]) -> Option<(&'a str, &'a str)> {
    for entry in eligible {
        if input.starts_with(entry.as_str()) {
            return Some(input.split_at(entry.len()));
        }
    }
    None
}

/// Decode an eligible escape into the verbatim text it represents
/// in a raw string. Eligible entries are constrained to the three
/// self-decoding escapes by [`is_supported_eligible_entry`], so the
/// decoded form is exactly the entry with its leading backslash
/// removed.
fn eliminable_decoded(escape: &str) -> &str {
    &escape['\\'.len_utf8()..]
}

/// Take any backslash escape from the front of `input`. Recognises
/// `\xNN` (4 bytes), `\u{...}` (variable length), and any
/// single-character escape (`\n`, `\t`, `\r`, `\0`, `\"`, `\\`,
/// `\'`, line continuation, …). Returns `None` if `input` does not
/// start with `\` or the escape is malformed (incomplete `\u{...}`
/// without a closing brace, dangling backslash at the end of input,
/// truncated `\xNN`).
fn take_escape_non_raw(input: &str) -> Option<(&str, &str)> {
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'\\') {
        return None;
    }
    let second_byte = *bytes.get(1)?;
    let escape_len = match second_byte {
        b'x' => 4,
        b'u' => {
            // `\u{...}`: scan to the closing `}`. The bytes between
            // `{` and `}` are constrained to ASCII hex by the rustc
            // lexer, so a byte-level scan is sufficient.
            let mut length: usize = 2;
            let mut closing_found = false;
            for &byte in &bytes[2..] {
                length = length.saturating_add(1);
                if byte == b'}' {
                    closing_found = true;
                    break;
                }
            }
            if !closing_found {
                return None;
            }
            length
        }
        _ => {
            // `\` + a single UTF-8 character (e.g. `\n`, `\"`,
            // or the line-continuation `\<newline>`).
            let second_char = input['\\'.len_utf8()..].chars().next()?;
            '\\'.len_utf8() + second_char.len_utf8()
        }
    };
    if escape_len > input.len() {
        return None;
    }
    Some(input.split_at(escape_len))
}

/// Take a single non-backslash UTF-8 character from the front of
/// `input`. Returns `None` only when `input` is empty or starts with
/// `\`, in which case the caller should run one of the escape
/// combinators first.
fn take_literal_char(input: &str) -> Option<(&str, &str)> {
    let first = input.chars().next()?;
    if first == '\\' {
        return None;
    }
    Some(input.split_at(first.len_utf8()))
}

/// Smallest number of `#` characters needed so that the closing
/// `"<n #s>` sequence does not appear inside `decoded`.
///
/// In practice this is 0 for paths and 1 for JSON / HTML snippets;
/// longer runs only matter when the literal itself embeds
/// raw-string source text.
fn minimal_hash_count(decoded: &str) -> usize {
    let mut hashes = String::new();
    let mut count: usize = 0;
    loop {
        let mut pattern = String::with_capacity('"'.len_utf8() + hashes.len());
        pattern.push('"');
        pattern.push_str(&hashes);
        if !decoded.contains(&pattern) {
            return count;
        }
        hashes.push('#');
        count = count.saturating_add(1);
    }
}

/// A supported `escapes_eligible` entry is one of the three Rust
/// escapes that self-decode — that is, whose decoded character is
/// exactly the byte that follows the backslash: `\"`, `\\`, `\'`.
/// `eliminable_decoded`'s contract is "strip the leading backslash",
/// which only holds for these three. Every other valid Rust escape
/// (`\n`, `\t`, `\r`, `\0`, `\xNN`, `\u{...}`) decodes to a
/// different character, so accepting it here would let the autofix
/// silently corrupt strings — e.g. `escapes_eligible = ["\\n"]`
/// would rewrite a newline-containing literal to one containing the
/// letter `n`.
///
/// The supported set is the same one named by
/// [`DEFAULT_ESCAPES_ELIGIBLE`]; matching against that constant
/// keeps the two definitions from drifting apart if a future
/// extension to `eliminable_decoded` ever adds a fourth entry.
fn is_supported_eligible_entry(entry: &str) -> bool {
    DEFAULT_ESCAPES_ELIGIBLE.contains(&entry)
}
