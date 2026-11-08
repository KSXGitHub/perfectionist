use crate::common::DefaultState;
use crate::enclosing_hir::emit_at_enclosing_hir;
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir::HirId;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

mod scan;
mod visibility;

use scan::Violation;
use visibility::Scope;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Forbids a visible character written as a numeric escape,
    /// `\xNN` or `\u{...}`, in a string, character, or C-string
    /// literal, and suggests the character itself.
    ///
    /// Visible means at least one terminal column wide, not
    /// whitespace, and not a Private Use Area codepoint. So
    /// `\u{2192}` is flagged, while a control character (`\u{0}`),
    /// whitespace (`\u{9}`, `\u{A0}`), a zero-width or format
    /// character (`\u{200B}`), a combining mark (`\u{301}`) and a
    /// private-use codepoint (`\u{E0B0}`) are not: none of those has
    /// a glyph a reader could pick out, which is where the escape is
    /// the clearer spelling. The `flag_non_ascii` knob narrows the
    /// set further, to ASCII.
    ///
    /// An escape also stays where the literal cannot hold its
    /// character verbatim: its own closing delimiter, and the
    /// backslash. Byte literals are out of scope, as is `\xNN` above
    /// `\x7F`, which denotes one byte rather than one character.
    ///
    /// The rewrite preserves the literal's value, a `format!`
    /// template included, since the placeholder grammar applies to
    /// the decoded text. Every literal the rule covers is scanned,
    /// including one inside a macro invocation and one a
    /// `#[cfg(...)]` excludes from the build; a file reached through
    /// `include_str!` is data rather than Rust, so it is skipped.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. An
    /// escape hides what the literal says: a reader has to decode
    /// `\u{2192}` to learn that the string holds an arrow, a reviewer
    /// cannot tell the arrow from any other codepoint at a glance,
    /// and a search for the character itself finds nothing. Spelling
    /// the character makes the literal read as the text it is, and
    /// keeps one spelling for a character wherever it appears. Source
    /// files are UTF-8, so the escape buys no portability in
    /// exchange.
    ///
    /// ### Interaction with sibling rules
    ///
    /// Spelling the character out can hand it to a rule that forbids
    /// the character itself, and the two then compose into a two-step
    /// fix rather than conflicting: a `\u{2026}` in a panic message
    /// is spelled out by this rule, which
    /// `perfectionist::unicode_ellipsis_in_panic_messages` then
    /// reports, so the pair lands on the ASCII three-dot form.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::invisible_characters` (`correctness`, deny by
    /// default) is this rule's mirror image. Where a character has no
    /// glyph of its own, such as a zero-width space or a soft hyphen,
    /// it reports the character and asks for the escape; this rule
    /// reports the escape and asks for the character. An invisible
    /// character is one this rule declines by definition, so the two
    /// never ask opposite things about the same character, and
    /// together they read as one policy: escape what has no glyph,
    /// spell out what has one.
    ///
    /// `clippy::unicode_not_nfc` (`pedantic`, off by default) reports
    /// a decomposed sequence and asks for the precomposed form. It
    /// reads the literal as written, so an escape anywhere in the
    /// sequence hides it: `"caf\u{65}"` followed by a combining acute
    /// goes unreported until the escape is spelled out, at which
    /// point Clippy sees `"cafe"` plus the mark and composes it. This
    /// rule never flags a combining mark, so it only ever hands that
    /// lint work it could not otherwise reach.
    ///
    /// `clippy::non_ascii_literal` (`restriction`, off by default) is
    /// the one that disagrees. It forbids a non-ASCII character
    /// written as itself and asks for the escape, which is the
    /// opposite request on the same literal. Running both is a matter
    /// of setting `flag_non_ascii = false`, which leaves this rule
    /// the ASCII half the two agree on, such as `\x41` for `A`, and
    /// leaves the non-ASCII escapes to Clippy.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let arrow = "\u{2192}";
    /// let name = "Ren\u{E9}";
    /// let letter = '\x41';
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let arrow = "→";
    /// let name = "René";
    /// let letter = 'A';
    /// ```
    pub perfectionist::NUMERIC_ESCAPE_FOR_VISIBLE_CHARACTER,
    Warn,
    "visible character written as a numeric escape; spell the character as itself",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::numeric_escape_for_visible_character";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Flag an escape whose character is outside ASCII, such as
    /// `\u{2192}` for `→`. Defaults to `true`. Set it to `false` to
    /// keep only the ASCII half of the rule, such as `\x41` for `A`.
    flag_non_ascii: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            flag_non_ascii: true,
        }
    }
}

pub struct NumericEscapeForVisibleCharacter {
    scope: Scope,
}

impl NumericEscapeForVisibleCharacter {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            scope: Scope {
                flag_non_ascii: config.flag_non_ascii,
            },
        }
    }
}

impl_lint_pass!(NumericEscapeForVisibleCharacter => [NUMERIC_ESCAPE_FOR_VISIBLE_CHARACTER]);

impl Register for rule::NumericEscapeForVisibleCharacter {
    /// A lexical test with a value-preserving rewrite and no
    /// threshold to tune: whether the character has a glyph is what
    /// decides, not a limit a project would want set differently. One
    /// policy does vary: whether a non-ASCII character may be spelled
    /// out at all. That is a knob rather than a reason to ship the
    /// rule inactive, since the ASCII half holds either way.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[NUMERIC_ESCAPE_FOR_VISIBLE_CHARACTER]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(NumericEscapeForVisibleCharacter::new())
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for NumericEscapeForVisibleCharacter {
    /// The scan is lexical, so it runs once over the crate's source
    /// files rather than per HIR node: a literal a macro consumes
    /// during expansion never reaches the HIR as a literal, and one
    /// behind a `#[cfg(...)]` never reaches it at all. Each finding is
    /// then anchored at the HIR node that lexically contains it, so a
    /// per-item `#[allow]` / `#[expect]` resolves.
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let violations = scan::collect_violations(lint_context, self.scope);
        emit_at_enclosing_hir(lint_context.tcx, violations, |hir_id, span, violation| {
            emit(lint_context, hir_id, span, violation);
        });
    }
}

/// Report one escape at `span`, suggesting the character it stands
/// for. The suggestion is machine-applicable: the escape and the
/// character decode to the same value, and the scan has already
/// ruled out the characters the surrounding literal cannot hold
/// verbatim.
fn emit(lint_context: &LateContext<'_>, hir_id: HirId, span: Span, violation: Violation) {
    let Violation { character, escape } = violation;
    span_lint_hir_and_then(
        lint_context,
        NUMERIC_ESCAPE_FOR_VISIBLE_CHARACTER,
        hir_id,
        span,
        format!(
            "visible character `{character}` (U+{:04X}) written as the escape `{escape}`",
            u32::from(character),
        ),
        |diag| {
            diag.span_suggestion(
                span,
                "spell the character as itself",
                character.to_string(),
                Applicability::MachineApplicable,
            );
        },
    );
}
