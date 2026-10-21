//! What one flagged dash carries from the scan that found it to the
//! late pass that emits it, paired there with the dash's own span.
//!
//! Every one of the rule's scans builds the same [`Finding`]. The
//! comment scan and the `expect` / `expect_err` scan hand theirs to
//! the emitter directly, both already running in the late pass; the
//! macro-argument scan runs pre-expansion and parks its findings in
//! the process-wide queue first. `rustc_span::Span` is
//! `Copy + Send + Sync` and `String` is `Send`, so that queue is safe
//! to hold in a `static`.

use super::scan::Dash;
use core::fmt::{self, Display, Formatter};

/// One dash to report, minus the span it was found at.
pub(super) struct Finding {
    pub(super) dash: Dash,
    pub(super) surface: Surface,
}

/// Where a dash was found, as the diagnostic names it.
#[derive(Clone)]
pub(super) enum Surface {
    /// A `///`, `//!`, `/** */` or `/*! */` doc comment.
    DocComment,
    /// A `//` or `/* */` comment.
    Comment,
    /// A string-literal argument of a call to the named macro, held
    /// without the trailing `!`.
    MacroMessage(String),
    /// The panic-message argument of a call to the named method.
    MethodMessage(String),
}

impl Display for Surface {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Surface::DocComment => formatter.write_str("doc comment"),
            Surface::Comment => formatter.write_str("comment"),
            Surface::MacroMessage(name) => write!(formatter, "`{name}!` message"),
            Surface::MethodMessage(name) => write!(formatter, "`{name}` message"),
        }
    }
}
