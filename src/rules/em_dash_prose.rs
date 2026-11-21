use crate::common::DefaultState;
use crate::macro_template::find_message_literals;
use crate::rule_index::{Register, rule};
use rustc_ast::{MacCall, Path};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;
use std::sync::{Mutex, PoisonError};

mod blockquote;
mod config;
mod emit;
mod finding;
mod late;
mod literal;
mod scan;

use config::{ResolvedConfig, resolved_config};
use finding::{Finding, Surface};
use late::EmDashProseLate;
use literal::scan_string_literal;
use scan::DashPolicy;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a dash standing between two clauses of a sentence:
    /// U+2014 EM DASH (`—`) in a doc comment, in a regular comment,
    /// or in the message of a macro or method that shows text to the
    /// program's user. U+2013 EN DASH (`–`) and the typewriter
    /// spelling `--` are flagged alongside it, each behind a knob of
    /// its own.
    ///
    /// A dash inside a quotation or a code region stays: the author
    /// is reproducing that text rather than writing it.
    ///
    /// No fix is offered, and that is the rule's point rather than an
    /// omission. Replacing the dash without rewording silences the
    /// warning and leaves the phrasing it reports in place.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue.
    /// Documentation and comments are technical writing, read for
    /// ease of understanding rather than for effect, and a dash
    /// standing between two clauses asks the reader to work out a
    /// relationship the sentence never states. A comma, a semicolon,
    /// a colon or a sentence break each state it instead.
    ///
    /// The dash is a symptom more than the fault. A sentence that
    /// reaches for one is usually carrying more than one idea, so the
    /// mark reveals the problem rather than causing it, and
    /// one idea per sentence is what makes the passage read easily.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// /// Walks the tree — including hidden directories — and
    /// /// returns the total size.
    /// fn walk() {}
    ///
    /// println!("Skipping path — {path:?}");
    /// let config = load().expect("no config — run `init` first");
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// /// Walks the tree, hidden directories included. Returns the
    /// /// total size.
    /// fn walk() {}
    ///
    /// println!("Skipping path: {path:?}");
    /// let config = load().expect("no config. Run `init` first");
    /// ```
    pub perfectionist::EM_DASH_PROSE,
    Warn,
    "dash standing between two clauses; restructure the sentence instead",
    report_in_external_macro: false
}

/// Macros whose string-literal arguments carry prose the program
/// shows its user, each paired with the number of leading
/// comma-separated arguments that hold something else.
///
/// Each name is one path segment, matched against the final segment of
/// the invocation's path, so `info` covers `log::info!` and
/// `tracing::info!` alike. The count is what keeps program data out of
/// the scan: `assert_eq!`'s first two arguments are the values being
/// compared, so a string literal among them is data rather than a
/// message. A writer and a log level are not lone string literals to
/// begin with, so the macros that take one count nothing.
const TARGET_MACROS: &[(&str, usize)] = &[
    // Writers.
    ("print", 0),
    ("println", 0),
    ("eprint", 0),
    ("eprintln", 0),
    ("format", 0),
    ("write", 0),
    ("writeln", 0),
    // Panics.
    ("panic", 0),
    ("unimplemented", 0),
    ("todo", 0),
    ("unreachable", 0),
    ("debug_unreachable", 0),
    // Assertions: the condition, or the pair of compared values,
    // comes before the message.
    ("assert", 1),
    ("debug_assert", 1),
    ("assert_eq", 2),
    ("assert_ne", 2),
    ("debug_assert_eq", 2),
    ("debug_assert_ne", 2),
    // The `log` family.
    ("log", 0),
    ("error", 0),
    ("warn", 0),
    ("info", 0),
    ("debug", 0),
    ("trace", 0),
];

/// `Option` / `Result` methods whose first argument is a panic
/// message. The receiver's type is checked before the argument is read
/// as one, since both names are common enough elsewhere.
const MESSAGE_METHODS: &[&str] = &["expect", "expect_err"];

pub struct EmDashProse {
    scan_macro_messages: bool,
    policy: DashPolicy,
}

impl EmDashProse {
    fn new() -> Self {
        let ResolvedConfig {
            scan_macro_messages,
            policy,
            ..
        } = resolved_config();
        Self {
            scan_macro_messages,
            policy,
        }
    }
}

impl_lint_pass!(EmDashProse => [EM_DASH_PROSE]);
impl_lint_pass!(EmDashProseLate => [EM_DASH_PROSE]);

impl Register for rule::EmDashProse {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[EM_DASH_PROSE]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        // The comment surfaces and the `expect` / `expect_err`
        // messages are late-pass scans, which is also where every
        // diagnostic is emitted. The macro surface is read
        // pre-expansion, as `overly_long_print_macro` reads it: a
        // `format!`-family template is consumed by format-args
        // lowering, so the late pass never sees the literal, and the
        // argument tokens are only intact before expansion.
        lint_store.register_pre_expansion_lint_pass(Box::new(|| Box::new(EmDashProse::new())));
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(EmDashProseLate::new())));
    }
}

/// Dashes the pre-expansion pass has found in a macro argument,
/// waiting for the late pass to anchor each at its enclosing HIR node
/// and emit. See [`mod@finding`] for why a process-wide static is
/// safe.
static PENDING_VIOLATIONS: Mutex<Vec<(Span, Finding)>> = Mutex::new(Vec::new());

impl EarlyLintPass for EmDashProse {
    fn check_mac(&mut self, lint_context: &EarlyContext<'_>, mac_call: &MacCall) {
        if !self.scan_macro_messages {
            return;
        }
        let Some((macro_name, skip)) = target_macro(&mac_call.path) else {
            return;
        };
        let source_map = lint_context.sess().source_map();
        for literal_span in find_message_literals(&mac_call.args.tokens, skip) {
            let Ok(snippet) = source_map.span_to_snippet(literal_span) else {
                continue;
            };
            // The dash span is narrower than the node that produced
            // it, which is the shape `report_in_external_macro: false`
            // cannot filter. No extra guard is needed all the same: a
            // pre-expansion pass walks the crate as written and never
            // reaches an expansion's output, and `scan_string_literal`
            // bails on a snippet that is not the literal it claims to
            // be.
            for (span, dash) in scan_string_literal(&snippet, literal_span, self.policy) {
                queue((
                    span,
                    Finding {
                        dash,
                        surface: Surface::MacroMessage(macro_name.to_owned()),
                    },
                ));
            }
        }
    }
}

/// The final segment of `path` and its leading-argument count, where
/// that segment names one of [`TARGET_MACROS`].
fn target_macro(path: &Path) -> Option<(&str, usize)> {
    let name = path.segments.last()?.ident.name.as_str();
    TARGET_MACROS
        .iter()
        .find(|(target, _skip)| *target == name)
        .map(|&(_target, skip)| (name, skip))
}

fn queue(finding: (Span, Finding)) {
    let mut guard = PENDING_VIOLATIONS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    guard.push(finding);
}
