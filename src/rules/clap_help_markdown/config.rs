//! Configuration for `clap_help_markdown`: the set of forbidden
//! markdown constructs and the attribute keys that count as a help
//! override.

use crate::markdown::ConstructKind;
use std::collections::BTreeSet;

/// A markdown construct category the rule can be configured to forbid,
/// as it appears in the `extra_constructs` / `ignore_constructs` arrays
/// of `dylint.toml`. The coarse policy counterpart to the scanner's
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

/// The built-in default set of forbidden constructs — the conservative
/// constructs that read badly in a terminal `--help`. Emphasis and lists
/// are deliberately excluded; clap renders them as their literal
/// characters, which usually reads cleanly.
pub(super) const DEFAULT_FORBID: &[ConstructCategory] = &[
    ConstructCategory::Html,
    ConstructCategory::InlineLink,
    ConstructCategory::ReferenceLink,
    ConstructCategory::IntraDocLink,
    ConstructCategory::CodeBlock,
    ConstructCategory::CodeSpan,
    ConstructCategory::Heading,
];

/// The attribute keys that, when present inside a `clap` / `arg` /
/// `command` attribute, mean the doc comment is no longer the source of
/// truth for help text — so the lint stays silent. This is clap's own
/// complete set of help-text overrides, so it is fixed: there is nothing
/// for a project to add or remove.
pub(super) const OVERRIDE_KEYS: &[&str] = &["about", "long_about", "help", "long_help"];

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Markdown constructs to forbid in addition to the built-in default
    /// set (`html`, `inline_link`, `reference_link`, `intra_doc_link`,
    /// `code_block`, `code_span`, `heading`). Empty by default; `bold`,
    /// `italic`, and `list` are the usual additions — clap renders them
    /// acceptably, so they are off by default.
    pub(super) extra_constructs: Vec<ConstructCategory>,
    /// Constructs to drop from the forbidden set, even if they appear in
    /// the built-in defaults. Empty by default; applied after the merge
    /// with `extra_constructs`, so this knob always wins. Use it to
    /// permit a construct in help text, e.g.
    /// `ignore_constructs = ["code_span"]`.
    pub(super) ignore_constructs: Vec<ConstructCategory>,
    /// For projects that never let a doc comment become `--help` text,
    /// preferring an explicit clap override (`#[arg(help = "...")]`,
    /// `#[command(about = "...")]`, ...) on every command, argument, and
    /// value. When `true`, the rule flags every clap-derived doc comment
    /// that reaches `--help` without such an override, regardless of
    /// whether it contains markdown — the doc comment stays for
    /// `cargo doc`, but `--help` must come from a plain-string override.
    /// A node with no doc comment is left alone: the requirement is only
    /// that a *present* doc comment never feeds `--help` unoverridden, so
    /// a bare `#[arg(help = "...")]` with no `///` is fine. This
    /// supersedes the markdown scan — an override silences the markdown
    /// concern anyway — so an unoverridden doc comment is reported once as
    /// a missing override rather than per markdown construct. Defaults to
    /// `false`.
    pub(super) require_help_override: bool,
}

/// The resolved, runtime form of [`Config`]: the active forbidden-set as
/// a fast-lookup [`BTreeSet`], plus the `require_help_override` mode flag.
pub(super) struct ResolvedConfig {
    pub(super) forbid: BTreeSet<ConstructCategory>,
    /// Whether the stricter "every help-bound doc comment must be
    /// overridden" mode is active. See [`Config::require_help_override`].
    pub(super) require_help_override: bool,
}

impl ResolvedConfig {
    pub(super) fn from_config(config: Config) -> Self {
        // Effective set = (default ∪ extra_constructs) \ ignore_constructs
        // — the same add/subtract-over-a-curated-default shape as the
        // catalogue's `resolve_string_set` / `resolve_symbol_set` helpers.
        let mut forbid: BTreeSet<ConstructCategory> = DEFAULT_FORBID
            .iter()
            .copied()
            .chain(config.extra_constructs)
            .collect();
        for category in config.ignore_constructs {
            forbid.remove(&category);
        }
        Self {
            forbid,
            require_help_override: config.require_help_override,
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
