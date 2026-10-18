//! Configuration for `em_dash_prose`: the user-facing [`Config`]
//! shape and the [`ResolvedConfig`] both of the rule's passes read.

use super::scan::DashPolicy;

const CONFIG_KEY: &str = "perfectionist::em_dash_prose";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Scan doc comments (`///`, `//!`, `/** */`, `/*! */`).
    /// Defaults to `true`.
    scan_doc_comments: bool,
    /// Scan regular comments (`//`, `/* */`). Defaults to `true`.
    scan_regular_comments: bool,
    /// Scan the message of a writer, panic, assertion or logging
    /// macro, and of the `Option` / `Result` methods `expect` and
    /// `expect_err`. A call is recognised by the last segment of its
    /// path. Defaults to `true`.
    scan_macro_messages: bool,
    /// Flag U+2013 EN DASH (`–`) wherever U+2014 EM DASH is
    /// flagged. Defaults to `true`: the en dash reads as a narrower
    /// em dash in running prose and elides the same clause
    /// relationship. Turn it off in a project that writes numeric
    /// ranges as `10–20`.
    flag_en_dash: bool,
    /// Flag a run of two or more ASCII hyphens with a space on each
    /// side (`a -- b`), the typewriter spelling of an em dash.
    /// Defaults to `true`.
    ///
    /// The space on each side is what keeps `--help`, `-->`, `<!--`
    /// and a run that opens or closes a line out of the count. A run
    /// inside a quotation or a code region is left alone for the same
    /// reason the Unicode dashes are. Comments are the only surface
    /// this knob reaches: inside a string literal a `--` is as often
    /// another language's comment marker or an end-of-options
    /// separator as it is a dash, so string literals are left to the
    /// Unicode dashes whatever this is set to.
    flag_pseudo_em_dash: bool,
    /// Replacement for the diagnostic's help text, which otherwise
    /// asks for the sentence to be restructured. Set it to point at a
    /// house style guide, or to write the instruction in another
    /// language. Unset by default, and the note saying what a dash
    /// leaves the reader to work out stays in place either way.
    message: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scan_doc_comments: true,
            scan_regular_comments: true,
            scan_macro_messages: true,
            flag_en_dash: true,
            flag_pseudo_em_dash: true,
            message: None,
        }
    }
}

/// The rule's settings once parsed, shared by the pre-expansion pass
/// that scans macro arguments and the late pass that scans comments
/// and emits, so the two agree on which dashes count.
pub(super) struct ResolvedConfig {
    pub(super) scan_doc_comments: bool,
    pub(super) scan_regular_comments: bool,
    pub(super) scan_macro_messages: bool,
    pub(super) policy: DashPolicy,
    pub(super) message: Option<String>,
}

/// Read the rule's configuration from the shared [`CONFIG_KEY`] table.
/// Called once per pass, since each pass builds its own copy; the
/// result is not cached.
pub(super) fn resolved_config() -> ResolvedConfig {
    let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
    ResolvedConfig {
        scan_doc_comments: config.scan_doc_comments,
        scan_regular_comments: config.scan_regular_comments,
        scan_macro_messages: config.scan_macro_messages,
        policy: DashPolicy {
            en_dash: config.flag_en_dash,
            ascii_run: config.flag_pseudo_em_dash,
        },
        message: config.message,
    }
}
