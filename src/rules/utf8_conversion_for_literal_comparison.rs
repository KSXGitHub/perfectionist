use crate::common::{DefaultState, hir_in_external_macro, is_author_written_match};
use crate::rule_index::{Register, rule};
use rustc_hir::{BinOpKind, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

mod consumer;
mod emit;
mod source;

use consumer::{Consumer, literal_arms, literal_operand};
use source::Converted;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a UTF-8 conversion whose result only ever meets string
    /// literals. The converted value's own representation is not
    /// UTF-8: an OS string (`OsStr`, `OsString`, `Path`, `PathBuf`),
    /// a byte buffer (`[u8]`, `Vec<u8>`, `Box<[u8]>`), or a C string
    /// (`CStr`, `CString`).
    ///
    /// The conversions read are `to_str`, `to_string_lossy`,
    /// `OsString::into_string`, `str::from_utf8`, `String::from_utf8`
    /// and `String::from_utf8_lossy`, each optionally followed by
    /// `unwrap` or `expect`.
    ///
    /// Two consumers count as meeting only literals:
    ///
    /// - an `==` or `!=` whose other operand is a string literal, or
    ///   a `Some` or `Ok` of one, and
    /// - a `match` whose every arm is a string literal, a `Some` or
    ///   `Ok` of one, or a wildcard, with no arm binding the decoded
    ///   text.
    ///
    /// Left alone:
    ///
    /// - A comparand that is not a literal. There is nothing to
    ///   compare the native form against.
    /// - An arm that binds the decoded text, or a guard that reads the
    ///   binding. The text is being used as text, which is what the
    ///   conversion is for.
    /// - A `match` that gives the non-UTF-8 case an arm of its own. A
    ///   byte comparison tells a listed literal from everything else
    ///   and cannot tell ill-formed input from valid input that is
    ///   none of the literals, so the two have to share one wildcard.
    /// - A lossy conversion measured against a literal holding
    ///   U+FFFD. A lossy conversion maps each ill-formed sequence to
    ///   U+FFFD, so such a literal is matched by inputs whose bytes
    ///   differ from it and the rewrite would not preserve the answer.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. The
    /// comparison answers the same way either spelling, which is why
    /// the rule reports only where that holds. The objection is to
    /// what the reader has to carry:
    ///
    /// - **A wrapper no arm asked for.** A `match` deciding which of
    ///   three words a value is, written over `to_str`, makes every
    ///   arm carry a `Some` that answers a second question about
    ///   encoding. Spelling the arms as the words removes it.
    /// - **A validity test with no consumer.** Where the `None` or
    ///   `Err` case shares a wildcard with "some other string", the
    ///   conversion established something the code then declines to
    ///   use. A reader who meets `to_str` expects the encoding to
    ///   matter somewhere.
    ///
    /// Speed is not the argument. Byte equality compares lengths
    /// before bytes where a UTF-8 validation scans the whole input,
    /// but these values are short and read once, so a project moved
    /// by throughput should measure rather than take the shape on
    /// faith.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::cmp_owned` flags an owned value built for a
    /// comparison, such as `x == "foo".to_string()`. A `to_str`
    /// allocates nothing, so none of the shapes here reach it.
    /// `clippy::needless_as_bytes` covers `as_bytes` followed by
    /// `len` or `is_empty` and does not reach a comparison. Neither
    /// lint fires on this anti-pattern, so the rule takes its own
    /// name rather than mirroring one.
    ///
    /// ### Example
    ///
    /// ```rust,ignore
    /// // Bad: the `Option` is threaded through arms deciding between
    /// // three words.
    /// match value.to_str() {
    ///     Some("true") => Ok(true),
    ///     Some("false") | Some("") => Ok(false),
    ///     _ => Err(error),
    /// }
    ///
    /// // Bad: the equality asks about encoding on the way to asking
    /// // about one word.
    /// if value.to_str() == Some("true") {}
    /// ```
    ///
    /// Use instead:
    ///
    /// ```rust,ignore
    /// // Good: the arms are the words.
    /// match value.as_encoded_bytes() {
    ///     b"true" => Ok(true),
    ///     b"false" | b"" => Ok(false),
    ///     _ => Err(error),
    /// }
    ///
    /// // Good: an equality keeps its literal as written.
    /// if value == OsStr::new("true") {}
    /// ```
    pub perfectionist::UTF8_CONVERSION_FOR_LITERAL_COMPARISON,
    Warn,
    "UTF-8 conversion whose result only meets string literals; compare the native form"
}

const CONFIG_KEY: &str = "perfectionist::utf8_conversion_for_literal_comparison";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct Utf8ConversionForLiteralComparison;

impl_lint_pass!(Utf8ConversionForLiteralComparison => [UTF8_CONVERSION_FOR_LITERAL_COMPARISON]);

impl Register for rule::Utf8ConversionForLiteralComparison {
    /// The conditions are each a local property of one expression, and
    /// the two carrying false-positive risk, a bound arm and a
    /// distinguished non-UTF-8 outcome, are exclusions rather than
    /// heuristics. A site that satisfies all of them is a violation
    /// rather than a guess, so there is no baseline a project could
    /// want instead.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[UTF8_CONVERSION_FOR_LITERAL_COMPARISON]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        lint_store
            .register_late_lint_pass(Box::new(|_| Box::new(Utf8ConversionForLiteralComparison)));
    }
}

impl<'tcx> LateLintPass<'tcx> for Utf8ConversionForLiteralComparison {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let Some((consumed, converted, consumer)) = consumed_conversion(cx, expr) else {
            return;
        };
        // The span reported is the converted operand's rather than the
        // whole comparison's, because `matches!` builds the `match` it
        // is read through: that `match` carries an expansion span where
        // its scrutinee carries the author's. The operand is also what
        // the rewrite replaces, so the two agree.
        //
        // An operand a macro built is the macro author's rather than
        // the caller's, and the help would describe a rewrite of the
        // definition. `report_in_external_macro: false` covers only
        // another crate's macro, so a same-crate `macro_rules!` needs
        // this.
        if consumed.span.from_expansion() {
            return;
        }
        // A derive may stamp a user span on every token it synthesises,
        // which leaves the check above nothing to find; the enclosing
        // item's span is what still says where the expression lives.
        if hir_in_external_macro(cx, consumed.hir_id, consumed.span) {
            return;
        }
        emit::report(cx, consumed, &converted, &consumer);
    }
}

/// The conversion `expr` consumes against literals, and how it
/// consumes it.
///
/// Reading downward from the consumer is what keeps the trigger local:
/// the comparison or the `match` is the expression handed to
/// `check_expr`, so the conversion is one of its own children rather
/// than something to be traced from.
fn consumed_conversion<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<(&'tcx Expr<'tcx>, Converted<'tcx>, Consumer)> {
    match expr.kind {
        ExprKind::Binary(operator, left, right) => {
            if !matches!(operator.node, BinOpKind::Eq | BinOpKind::Ne) {
                return None;
            }
            // Either operand may be the literal, so both orders are
            // tried.
            let (consumed, converted, literal) =
                literal_operand(cx, left, right).or_else(|| literal_operand(cx, right, left))?;
            Some((consumed, converted, Consumer::Equality { literal }))
        }
        ExprKind::Match(scrutinee, arms, match_source) if is_author_written_match(match_source) => {
            let converted = source::converted(cx, scrutinee)?;
            let literals = literal_arms(arms)?;
            Some((scrutinee, converted, Consumer::Patterns { literals }))
        }
        _ => None,
    }
}
