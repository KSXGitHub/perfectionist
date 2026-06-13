//! Configuration for `long_splittable_print_macro`: the user-facing [`Config`]
//! shape, the curated built-in `target_macros` list, and the in-memory
//! [`LongSplittablePrintMacro`] state the pre-expansion pass holds.
//!
//! Only the `line_continuation` rewrite ships today, so the `style`
//! knob documented in `planned-rules/long-splittable-print-macro.md` is
//! deliberately absent — a one-variant `style` enum would carry no
//! information. It returns when the `multiple_calls` half lands.

use crate::macro_path::{matches_any, parse_path_list};
use rustc_ast::Path;
use std::collections::BTreeSet;

const CONFIG_KEY: &str = "perfectionist::long_splittable_print_macro";

/// Source-line width, in display columns, at or below which a macro
/// invocation is left alone. Matches rustfmt's column default.
const DEFAULT_MAX_LINE_WIDTH: usize = 100;

/// Macros eligible for folding. Every entry is a pure side-effect
/// producer whose output is byte-equivalent before and after the
/// template is folded across lines. Macros that *return a value*
/// (`format!`, `format_args!`) or *terminate* (`panic!`, `assert!`,
/// the `debug_assert*` family, ...) are deliberately absent: see the
/// "Why not `format!`-family" section of
/// `planned-rules/long-splittable-print-macro.md`.
///
/// The list covers three groups: the stdout / stderr writers
/// (`println!`, `eprintln!`, `print!`, `eprint!`), the `Write` writers
/// (`writeln!`, `write!`), and the `log` family (`log!`, `error!`,
/// `warn!`, `info!`, `debug!`, `trace!`). The `log` family is matched
/// by final path segment, so `error` covers `log::error!` and
/// `tracing::error!` alike. A project that wants to fold a
/// differently-named macro replaces this whole list via the
/// `target_macros` config key.
const DEFAULT_TARGET_MACROS: &[&str] = &[
    "println", "eprintln", "print", "eprint", "writeln", "write", "log", "error", "warn", "info",
    "debug", "trace",
];

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Source-line width that triggers the rule. The width is the
    /// Unicode *display* width of the line containing the macro
    /// invocation, not its byte length, so a line of CJK text is
    /// measured the way a terminal renders it. Common alternatives to
    /// the default `100` are `80` (terminal) or `120` (wide editors).
    pub max_line_width: usize,
    /// Macros eligible for folding, each a `"a::b::c"`-style path (no
    /// trailing `!`). A single-segment entry matches by the
    /// invocation's final segment (so `"info"` covers `log::info!`);
    /// a multi-segment entry tail-matches the invocation path.
    /// Replaces the built-in list wholesale when present.
    pub target_macros: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_line_width: DEFAULT_MAX_LINE_WIDTH,
            target_macros: DEFAULT_TARGET_MACROS
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
        }
    }
}

pub(super) struct LongSplittablePrintMacro {
    max_line_width: usize,
    target_macros: BTreeSet<Vec<String>>,
}

impl LongSplittablePrintMacro {
    pub(super) fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        // An entry that parses to no segments (empty / whitespace-only)
        // is dropped by `parse_path_list`. A `target_macros = []`
        // therefore silently disables the rule, which is a reasonable
        // way to turn it off without touching `[perfectionist].disable`.
        let target_macros = if config.target_macros.is_empty() {
            BTreeSet::new()
        } else {
            parse_path_list(&config.target_macros)
        };
        Self {
            max_line_width: config.max_line_width,
            target_macros,
        }
    }

    pub(super) fn max_line_width(&self) -> usize {
        self.max_line_width
    }

    /// Whether the invocation path matches a configured target macro.
    pub(super) fn should_check_path(&self, path: &Path) -> bool {
        matches_any(path, &self.target_macros)
    }
}
