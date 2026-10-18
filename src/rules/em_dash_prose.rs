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
    /// U+2014 EM DASH in a doc comment, in a regular comment, or in
    /// the message of a macro or method that shows text to the
    /// program's user. U+2013 EN DASH and the typewriter spelling
    /// `--` are flagged alongside it, each behind a knob of its own.
    ///
    /// The messages scanned are those of the writers `print!`,
    /// `println!`, `eprint!`, `eprintln!`, `format!`, `write!` and
    /// `writeln!`; of the panics `panic!`, `unimplemented!`, `todo!`,
    /// `unreachable!` and `debug_unreachable!`; of the assertions
    /// `assert!`, `assert_eq!`, `assert_ne!`, `debug_assert!`,
    /// `debug_assert_eq!` and `debug_assert_ne!`; of the
    /// [`log`](https://docs.rs/log) family `log!`, `error!`, `warn!`,
    /// `info!`, `debug!` and `trace!`; and of the `Option` / `Result`
    /// methods `expect` and `expect_err`. A call matches by the final
    /// segment of its macro path, so `std::println!` and
    /// [`tracing::info!`](https://docs.rs/tracing) match too.
    ///
    /// Only an argument that is a string literal on its own is
    /// scanned, and only past the arguments that hold something other
    /// than a message, which together keep program data out:
    /// `write!`'s writer, `log!`'s level and `assert_eq!`'s pair of
    /// compared values are all left alone. So is a dash written as a
    /// `\u{2014}` escape rather than as the character itself, and one
    /// in a literal nested inside another macro call.
    ///
    /// The ASCII `--` is reported in comments only. Inside a string
    /// literal it is as often another language's comment marker or an
    /// end-of-options separator as it is a dash, so a literal is
    /// scanned for the Unicode dashes alone.
    ///
    /// A quotation keeps its dashes, on every surface the rule
    /// scans. The author of the surrounding sentence cannot
    /// restructure words they are only reproducing, so a span between
    /// a pair of quotation marks is left alone: the ASCII `'` and
    /// `"`, the backtick, and the typographic pairs
    /// `‘` / `’`, `“` / `”`, `‚` / `‘`, `„` / `“`,
    /// `«` / `»`, `‹` / `›`, `「` / `」` and `『` / `』`.
    /// An apostrophe opens nothing, so `don't` masks no prose, and a
    /// quotation has to close on the line it opened.
    ///
    /// A blockquote in a doc comment is a quotation at block level
    /// and keeps its dashes too. The exception is an alert, a block
    /// whose first line is a bracketed-bang name such as `[!NOTE]` or
    /// `[!WARNING]`: that is an aside in the author's own voice
    /// borrowing the blockquote syntax, so it is scanned like any
    /// other prose. A plain `//` comment is not markdown, so a
    /// leading `>` there delimits nothing.
    ///
    /// A code block, fenced or indented, keeps its dashes in either
    /// comment kind for the same reason a backtick span does: a dash
    /// there belongs to the code being quoted rather than to a
    /// sentence, so `--` inside a shell command and `--` opening a
    /// comment in a fenced Lua block are both left alone.
    ///
    /// No fix is offered, and that is the rule's point rather than an
    /// omission. Swapping the dash for a comma or a colon silences the
    /// warning and leaves the phrasing it reports in place, so the
    /// diagnostic carries prose only and no tool can rewrite the
    /// sentence mechanically.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. An em
    /// dash glues two clauses together without committing to a
    /// relationship between them, and that is the register
    /// AI-generated prose settles into: a writing assistant reaches
    /// for the dash whenever the structure of a sentence has not been
    /// decided. A comma, a semicolon, a colon or a sentence break
    /// each name the relationship instead, which is why an author
    /// writing carefully rarely picks the dash over one of them.
    ///
    /// The warning asks for the structural work the dash was eliding,
    /// which is why it suggests nothing: the sentence usually wants
    /// splitting into shorter complete ones, not a different
    /// punctuation character in the same slot.
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
