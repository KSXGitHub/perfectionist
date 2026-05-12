//! Configuration for `macro_argument_binding`. Owns the `Mode` enum,
//! the user-facing `Config` shape, the curated built-in deny / allow
//! lists, and the in-memory `MacroArgumentBinding` state the early
//! pass holds.
//!
//! Path-set construction goes through the helpers at the bottom of
//! this file so each list (`deny`, `allow`, `allow_extra`, `ignore`)
//! is built consistently.

use std::collections::BTreeSet;

use rustc_ast::MacCall;

use crate::macro_path::{matches_any, parse_path};

pub(super) const CONFIG_KEY: &str = "perfectionist::macro_argument_binding";

/// Macros whose argument list is checked unconditionally because the
/// expansion is known to evaluate captures conditionally on a `cfg`
/// (`debug_assert*`) or to drop them entirely in release builds.
const BUILTIN_DENY: &[&str] = &["debug_assert", "debug_assert_eq", "debug_assert_ne"];

/// Macros known to evaluate every top-level argument exactly once. The
/// list mirrors the curated set in `macro_trailing_comma`, with the
/// conditional-evaluation families (`log::*`, `tracing::*`) removed
/// because those *do* drop arguments below the configured filter level.
const BUILTIN_ALLOW: &[&str] = &[
    "format",
    "format_args",
    "print",
    "println",
    "eprint",
    "eprintln",
    "write",
    "writeln",
    "vec",
    "panic",
    "unimplemented",
    "todo",
    "unreachable",
    "assert",
    "assert_eq",
    "assert_ne",
    "matches",
    "dbg",
    "anyhow",
];

/// Eligibility mode. The default is `AllowAndDeny`. The matcher-based
/// mode described in `planned-rules/macro-argument-binding.md` is not
/// yet implemented and is therefore not exposed as a value here; a
/// `dylint.toml` that names it will fail to deserialise with a
/// useful error.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Mode {
    /// Flag only invocations of the curated deny list (`debug_assert*`
    /// plus `deny_extra`). Every other macro is silently accepted.
    DenyOnly,
    /// Flag every function-like or array-like invocation that carries
    /// a non-trivial top-level argument, regardless of any built-in
    /// classification — unless the invocation matches an `allow_extra`
    /// entry. The built-in allow list is deliberately ignored in this
    /// mode; project exceptions go in `allow_extra`.
    Blanket,
    /// Curated deny list plus curated allow list, both extensible via
    /// `deny_extra` / `allow_extra`. Macros on neither list are
    /// flagged — flagging unrecognised macros is deliberate so the
    /// rule remains useful in projects that depend on uncatalogued
    /// proc macros.
    #[default]
    AllowAndDeny,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub(super) struct Config {
    /// Master on/off switch for the rule. Defaults to `true`. Set
    /// to `false` to silence every diagnostic this lint would emit
    /// without having to enumerate every macro under `ignore`.
    pub enabled: bool,
    /// Eligibility mode. See [`Mode`].
    pub mode: Mode,
    /// Macros added to the built-in deny list. Each entry is a
    /// fully-qualified macro path (no trailing `!`) or a bare macro
    /// name to match by final segment only.
    pub deny_extra: Vec<String>,
    /// Macros added to the built-in allow list. Same matching rules
    /// as `deny_extra`. Only meaningful in `AllowAndDeny` and
    /// `Blanket` modes; in `DenyOnly` the allow list is unused.
    pub allow_extra: Vec<String>,
    /// Macros to skip entirely, regardless of which list they would
    /// otherwise hit. Same matching rules as `deny_extra`.
    pub ignore: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: Mode::default(),
            deny_extra: Vec::new(),
            allow_extra: Vec::new(),
            ignore: Vec::new(),
        }
    }
}

pub struct MacroArgumentBinding {
    pub(super) enabled: bool,
    mode: Mode,
    /// Built-in deny list plus `deny_extra`. Used in `DenyOnly` and
    /// `AllowAndDeny`.
    deny: BTreeSet<Vec<String>>,
    /// Built-in allow list plus `allow_extra`. Used only in
    /// `AllowAndDeny`; `Blanket` deliberately ignores the built-in
    /// allow list and consults `allow_extra` alone.
    allow: BTreeSet<Vec<String>>,
    /// Only the user-supplied `allow_extra` entries. Used in
    /// `Blanket` mode, which has no built-in allow list per the rule
    /// docs (`planned-rules/macro-argument-binding.md`).
    allow_extra: BTreeSet<Vec<String>>,
    pub(super) ignore: BTreeSet<Vec<String>>,
}

impl MacroArgumentBinding {
    pub(super) fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let extra_deny = parse_path_list(&config.deny_extra);
        let extra_allow = parse_path_list(&config.allow_extra);
        let deny = merge_with_builtins(BUILTIN_DENY, &extra_deny);
        let allow = merge_with_builtins(BUILTIN_ALLOW, &extra_allow);
        let ignore = parse_path_list(&config.ignore);
        Self {
            enabled: config.enabled,
            mode: config.mode,
            deny,
            allow,
            allow_extra: extra_allow,
            ignore,
        }
    }

    pub(super) fn arguments_should_be_checked(&self, mac_call: &MacCall) -> bool {
        let on_deny = matches_any(&mac_call.path, &self.deny);
        match self.mode {
            Mode::DenyOnly => on_deny,
            Mode::Blanket => !matches_any(&mac_call.path, &self.allow_extra),
            Mode::AllowAndDeny => on_deny || !matches_any(&mac_call.path, &self.allow),
        }
    }
}

fn parse_path_list(raw_entries: &[String]) -> BTreeSet<Vec<String>> {
    raw_entries
        .iter()
        .map(|entry| parse_path(entry))
        .filter(|parsed| !parsed.is_empty())
        .collect()
}

fn merge_with_builtins(builtin: &[&str], extras: &BTreeSet<Vec<String>>) -> BTreeSet<Vec<String>> {
    builtin
        .iter()
        .map(|name| vec![(*name).to_owned()])
        .chain(extras.iter().cloned())
        .collect()
}
