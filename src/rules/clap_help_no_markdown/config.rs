//! Configuration for `clap_help_no_markdown`: the set of forbidden
//! markdown constructs and the attribute keys that count as a help
//! override.

use crate::markdown::ConstructKind;
use rustc_span::Symbol;
use std::collections::BTreeSet;

/// A markdown construct category the rule can be configured to forbid,
/// as it appears in the `forbid` / `extra_forbid` arrays of
/// `dylint.toml`. The coarse policy counterpart to the scanner's
/// fine-grained [`ConstructKind`]: several kinds map onto one category
/// via [`ConstructCategory::from_kind`] — `reference_link` covers both a
/// `[text][id]` link and its `[id]: dest` definition, and an autolink
/// maps to nothing (it is never forbidden).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ConstructCategory {
    /// Raw HTML tags (`<br>`, `<code>`, `<a href="...">`, ...).
    Html,
    /// Inline links: `[text](https://example.com)`.
    InlineLink,
    /// Reference links (`[text][id]`) and their `[id]: ...`
    /// definitions.
    ReferenceLink,
    /// Intra-doc links: `` [`Type`] `` and `[Type]`.
    IntraDocLink,
    /// Fenced, `~~~`-fenced, or four-space-indented code blocks.
    CodeBlock,
    /// Inline code spans: `` `value` ``.
    CodeSpan,
    /// ATX (`# Heading`) and Setext (`Heading\n=====`) headings.
    Heading,
    /// `**bold**` / `__bold__` strong emphasis.
    Bold,
    /// `*italic*` / `_italic_` emphasis.
    Italic,
    /// Bullet and ordered list markers.
    List,
}

impl ConstructCategory {
    /// The category a scanned [`ConstructKind`] belongs to, or `None`
    /// for a kind that is never forbidden (an autolink).
    pub(super) fn from_kind(kind: ConstructKind) -> Option<Self> {
        Some(match kind {
            ConstructKind::CodeSpan => ConstructCategory::CodeSpan,
            ConstructKind::CodeBlock => ConstructCategory::CodeBlock,
            ConstructKind::InlineLink => ConstructCategory::InlineLink,
            ConstructKind::ReferenceLink | ConstructKind::ReferenceDefinition => {
                ConstructCategory::ReferenceLink
            }
            ConstructKind::IntraDocLink => ConstructCategory::IntraDocLink,
            ConstructKind::HtmlTag => ConstructCategory::Html,
            ConstructKind::Heading => ConstructCategory::Heading,
            ConstructKind::Bold => ConstructCategory::Bold,
            ConstructKind::Italic => ConstructCategory::Italic,
            ConstructKind::List => ConstructCategory::List,
            ConstructKind::Autolink => return None,
        })
    }

    /// A short noun phrase naming the construct in a diagnostic.
    pub(super) fn label(self) -> &'static str {
        match self {
            ConstructCategory::Html => "an HTML tag",
            ConstructCategory::InlineLink => "an inline link",
            ConstructCategory::ReferenceLink => "a reference link",
            ConstructCategory::IntraDocLink => "an intra-doc link",
            ConstructCategory::CodeBlock => "a code block",
            ConstructCategory::CodeSpan => "a code span",
            ConstructCategory::Heading => "a heading",
            ConstructCategory::Bold => "bold text",
            ConstructCategory::Italic => "italic text",
            ConstructCategory::List => "a list marker",
        }
    }
}

/// The default `forbid` set — the conservative constructs that read
/// badly in a terminal `--help`. Emphasis and lists are deliberately
/// excluded; clap renders them as their literal characters, which
/// usually reads cleanly.
pub(super) const DEFAULT_FORBID: &[ConstructCategory] = &[
    ConstructCategory::Html,
    ConstructCategory::InlineLink,
    ConstructCategory::ReferenceLink,
    ConstructCategory::IntraDocLink,
    ConstructCategory::CodeBlock,
    ConstructCategory::CodeSpan,
    ConstructCategory::Heading,
];

/// Default attribute keys that, when present inside a `clap` / `arg` /
/// `command` attribute, mean the doc comment is no longer the source of
/// truth for help text — so the lint stays silent.
pub(super) const DEFAULT_OVERRIDE_KEYS: &[&str] = &["about", "long_about", "help", "long_help"];

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Constructs to flag in addition to the default set (`html`,
    /// `inline_link`, `reference_link`, `intra_doc_link`, `code_block`,
    /// `code_span`, `heading`). Empty by default. The additions clap
    /// renders acceptably and so leaves off by default are `bold`,
    /// `italic`, and `list`.
    pub(super) extra_forbid: Vec<ConstructCategory>,
    /// Constructs to leave unflagged even though they are in the default
    /// set. Empty by default. Use it to permit a default construct in
    /// help text, e.g. `allow = ["code_span"]` to allow inline code
    /// spans. Applied after `extra_forbid`, so listing the same
    /// construct in both leaves it allowed.
    pub(super) allow: Vec<ConstructCategory>,
    /// Attribute keys (inside `#[clap(...)]`, `#[arg(...)]`, or
    /// `#[command(...)]`) that disable the lint for the documented item
    /// because they override the help text with a plain string.
    /// Defaults to `about`, `long_about`, `help`, and `long_help`.
    pub(super) override_keys: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            extra_forbid: Vec::new(),
            allow: Vec::new(),
            override_keys: DEFAULT_OVERRIDE_KEYS
                .iter()
                .map(|key| (*key).to_owned())
                .collect(),
        }
    }
}

/// The resolved, runtime form of [`Config`]: the active forbid set as a
/// fast-lookup [`BTreeSet`] and the override keys interned as
/// [`Symbol`]s.
pub(super) struct ResolvedConfig {
    pub(super) forbid: BTreeSet<ConstructCategory>,
    pub(super) override_keys: BTreeSet<Symbol>,
}

impl ResolvedConfig {
    pub(super) fn from_config(config: Config) -> Self {
        // Effective set = (default ∪ extra_forbid) \ allow — the same
        // add/subtract-over-a-curated-default shape as the catalogue's
        // `resolve_string_set` / `resolve_symbol_set` helpers.
        let mut forbid: BTreeSet<ConstructCategory> = DEFAULT_FORBID
            .iter()
            .copied()
            .chain(config.extra_forbid)
            .collect();
        for category in config.allow {
            forbid.remove(&category);
        }
        let override_keys = config
            .override_keys
            .iter()
            .map(|key| Symbol::intern(key))
            .collect();
        Self {
            forbid,
            override_keys,
        }
    }

    /// Whether the classifier needs to look for `*` / `_` emphasis runs
    /// — only when `bold` or `italic` is forbidden.
    pub(super) fn detect_emphasis(&self) -> bool {
        self.forbid.contains(&ConstructCategory::Bold)
            || self.forbid.contains(&ConstructCategory::Italic)
    }

    /// Whether the classifier needs to look for list markers.
    pub(super) fn detect_lists(&self) -> bool {
        self.forbid.contains(&ConstructCategory::List)
    }
}
