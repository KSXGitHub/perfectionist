//! Configuration for `clap_help_no_markdown`: the set of forbidden
//! markdown constructs and the attribute keys that count as a help
//! override.

use crate::markdown::ConstructKind;
use rustc_span::Symbol;
use std::collections::BTreeSet;

/// A user-facing "forbidden construct" category, as it appears in the
/// `forbid` / `extra_forbid` arrays of `dylint.toml`. Several
/// [`ConstructKind`]s map onto one category — `reference_link` covers
/// both a `[text][id]` link and its `[id]: dest` definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ForbidConstruct {
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

impl ForbidConstruct {
    /// The category a scanned [`ConstructKind`] belongs to, or `None`
    /// for a kind that is never forbidden (an autolink).
    pub(super) fn from_kind(kind: ConstructKind) -> Option<Self> {
        Some(match kind {
            ConstructKind::CodeSpan => ForbidConstruct::CodeSpan,
            ConstructKind::CodeBlock => ForbidConstruct::CodeBlock,
            ConstructKind::InlineLink => ForbidConstruct::InlineLink,
            ConstructKind::ReferenceLink | ConstructKind::ReferenceDefinition => {
                ForbidConstruct::ReferenceLink
            }
            ConstructKind::IntraDocLink => ForbidConstruct::IntraDocLink,
            ConstructKind::HtmlTag => ForbidConstruct::Html,
            ConstructKind::Heading => ForbidConstruct::Heading,
            ConstructKind::Bold => ForbidConstruct::Bold,
            ConstructKind::Italic => ForbidConstruct::Italic,
            ConstructKind::List => ForbidConstruct::List,
            ConstructKind::Autolink => return None,
        })
    }

    /// A short noun phrase naming the construct in a diagnostic.
    pub(super) fn label(self) -> &'static str {
        match self {
            ForbidConstruct::Html => "an HTML tag",
            ForbidConstruct::InlineLink => "an inline link",
            ForbidConstruct::ReferenceLink => "a reference link",
            ForbidConstruct::IntraDocLink => "an intra-doc link",
            ForbidConstruct::CodeBlock => "a code block",
            ForbidConstruct::CodeSpan => "a code span",
            ForbidConstruct::Heading => "a heading",
            ForbidConstruct::Bold => "bold text",
            ForbidConstruct::Italic => "italic text",
            ForbidConstruct::List => "a list marker",
        }
    }
}

/// The default `forbid` set — the conservative constructs that read
/// badly in a terminal `--help`. Emphasis and lists are deliberately
/// excluded; clap renders them as their literal characters, which
/// usually reads cleanly.
pub(super) const DEFAULT_FORBID: &[ForbidConstruct] = &[
    ForbidConstruct::Html,
    ForbidConstruct::InlineLink,
    ForbidConstruct::ReferenceLink,
    ForbidConstruct::IntraDocLink,
    ForbidConstruct::CodeBlock,
    ForbidConstruct::CodeSpan,
    ForbidConstruct::Heading,
];

/// Default attribute keys that, when present inside a `clap` / `arg` /
/// `command` attribute, mean the doc comment is no longer the source of
/// truth for help text — so the lint stays silent.
pub(super) const DEFAULT_OVERRIDE_KEYS: &[&str] = &["about", "long_about", "help", "long_help"];

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Constructs to flag. Defaults to the conservative set: `html`,
    /// `inline_link`, `reference_link`, `intra_doc_link`, `code_block`,
    /// `code_span`, and `heading`.
    pub(super) forbid: Vec<ForbidConstruct>,
    /// Additional constructs to flag on top of `forbid`. Empty by
    /// default; the available extras are `bold`, `italic`, and `list`,
    /// which clap renders as their literal characters and so are not
    /// flagged unless a project opts in.
    pub(super) extra_forbid: Vec<ForbidConstruct>,
    /// Attribute keys (inside `#[clap(...)]`, `#[arg(...)]`, or
    /// `#[command(...)]`) that disable the lint for the documented item
    /// because they override the help text with a plain string.
    /// Defaults to `about`, `long_about`, `help`, and `long_help`.
    pub(super) override_keys: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            forbid: DEFAULT_FORBID.to_vec(),
            extra_forbid: Vec::new(),
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
    pub(super) forbid: BTreeSet<ForbidConstruct>,
    pub(super) override_keys: BTreeSet<Symbol>,
}

impl ResolvedConfig {
    pub(super) fn from_config(config: Config) -> Self {
        let forbid: BTreeSet<ForbidConstruct> = config
            .forbid
            .into_iter()
            .chain(config.extra_forbid)
            .collect();
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
        self.forbid.contains(&ForbidConstruct::Bold)
            || self.forbid.contains(&ForbidConstruct::Italic)
    }

    /// Whether the classifier needs to look for list markers.
    pub(super) fn detect_lists(&self) -> bool {
        self.forbid.contains(&ForbidConstruct::List)
    }
}
