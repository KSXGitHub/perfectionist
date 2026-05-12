//! Configuration for `macro_trailing_comma`. Owns the user-facing
//! `Config` shape, the curated built-in name list, and the in-memory
//! `MacroTrailingComma` state the early pass holds.

use std::collections::BTreeSet;

use crate::macro_path::{matches_any, merge_with_builtins, parse_path, parse_path_list};

const CONFIG_KEY: &str = "perfectionist::macro_trailing_comma";

/// Curated macros whose top-level argument list is comma-separated with
/// a syntactically optional trailing comma. See the rule docs in
/// `planned-rules/macro-trailing-comma.md` for the inclusion criterion.
///
/// Each entry is a single segment; matching is by the final segment of
/// the invocation's path, so `vec!`, `std::vec!`, and `::std::vec!` all
/// match the `"vec"` entry.
const BUILTIN_NAME_BASED: &[&str] = &[
    // `core` / `std`
    "vec",
    "format",
    "format_args",
    "print",
    "println",
    "eprint",
    "eprintln",
    "write",
    "writeln",
    "panic",
    "unimplemented",
    "todo",
    "unreachable",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "matches",
    "dbg",
    "concat",
    "env",
    "option_env",
    // `pretty_assertions` (its `assert_eq` / `assert_ne` final segments
    // already match the `core` entries; `assert_str_eq` is unique to it).
    "assert_str_eq",
    // `maplit`
    "hashmap",
    "btreemap",
    "hashset",
    "btreeset",
    "convert_args",
    // `log` (its `error` / `warn` / `info` / `debug` / `trace` final
    // segments also cover `tracing`'s same-named macros).
    "log",
    "error",
    "warn",
    "info",
    "debug",
    "trace",
    // `tracing`
    "event",
    "span",
    // `anyhow`
    "anyhow",
    "bail",
    "ensure",
];

#[derive(Debug, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {
    /// Master on/off switch for the rule. Defaults to `true`. Set
    /// to `false` to silence every diagnostic this lint would emit
    /// without having to enumerate every macro under `ignore`.
    enabled: bool,
    /// Accepted for forward compatibility with the matcher-based half of
    /// the rule. Currently a no-op — only name-based eligibility is
    /// implemented; see `planned-rules/macro-trailing-comma.md` for the
    /// status breakdown.
    matcher_based: bool,
    /// Additional macro paths to treat as name-based eligible, on top
    /// of the curated built-in list. Each entry is matched by its
    /// final path segment, so `"my_crate::vec_like"` and `"vec_like"`
    /// both target invocations whose last segment is `vec_like`.
    /// Empty by default. Only add macros whose trailing comma is
    /// syntactically optional at the top level; macros that treat
    /// the comma as a fully optional separator throughout (rather
    /// than only at the tail) should not be listed here.
    extra_name_based: Vec<String>,
    /// Macro paths to opt out of the rule, even if they would
    /// otherwise be eligible via the built-in list or
    /// `extra_name_based`. Matched by final path segment, like
    /// `extra_name_based`. Checked first, so this knob always wins
    /// over eligibility. Empty by default.
    ignore: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            matcher_based: true,
            extra_name_based: Vec::new(),
            ignore: Vec::new(),
        }
    }
}

pub(super) struct MacroTrailingComma {
    enabled: bool,
    // TODO(matcher_based): the lookup is currently linear
    // (`entries.iter().any(...)`), so the `BTreeSet` ordering is unused
    // — it only deduplicates identical config entries. When the
    // matcher-based half grows these lists, bucket by entry length: a
    // `BTreeSet<String>` for single-segment entries (O(log N) on the
    // invocation's final segment) plus a `Vec<Vec<String>>` for
    // multi-segment entries.
    name_based: BTreeSet<Vec<String>>,
    ignore: BTreeSet<Vec<String>>,
}

impl MacroTrailingComma {
    pub(super) fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let extra_name_based: BTreeSet<Vec<String>> = config
            .extra_name_based
            .iter()
            .map(|entry| parse_path(entry))
            .filter(|parsed| !parsed.is_empty())
            .collect();
        let name_based = merge_with_builtins(BUILTIN_NAME_BASED, &extra_name_based);
        let ignore = parse_path_list(&config.ignore);
        Self {
            enabled: config.enabled,
            name_based,
            ignore,
        }
    }

    /// Path-side eligibility: combines the `enabled` switch with the
    /// `ignore` / built-in `name_based` lookup. Does *not* consider the
    /// invocation's argument shape — that stays in the early pass.
    pub(super) fn should_check_path(&self, path: &rustc_ast::Path) -> bool {
        self.enabled && !matches_any(path, &self.ignore) && matches_any(path, &self.name_based)
    }
}
