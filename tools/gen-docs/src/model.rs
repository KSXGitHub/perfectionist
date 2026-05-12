//! Data types threaded between the `extract` and `render` halves
//! of the pipeline. Kept here so neither half has to depend on the
//! other to name a `Rule` or a `ConfigDoc`.

use std::path::PathBuf;

use strum::{Display, EnumString};

/// The tool namespace every `perfectionist` lint registers under,
/// including the trailing `::`. Shared between the extractor (which
/// prepends it to build `Rule::namespaced`) and the renderer (which
/// strips it for the index column and re-emits it as a separately
/// styled span in each rule's heading).
pub(crate) const NAMESPACE: &str = "perfectionist::";

/// Page-global context threaded through the renderer. Grouping
/// these here keeps the per-article and per-field rendering
/// signatures from collecting more positional string args every
/// time a new contextual field is added.
pub(crate) struct RenderContext<'a> {
    /// Latest released version of the crate, read from `Cargo.toml`.
    pub(crate) crate_version: &'a str,
    /// Git ref the rendered "Source:" links target.
    pub(crate) git_ref: &'a str,
    /// Project repository URL, used for the README link in the
    /// banner and as the base for the per-rule blob URLs.
    pub(crate) repo_url: &'a str,
}

/// One lint, in the shape the page needs to render.
pub(crate) struct Rule {
    /// `perfectionist::flat_module_pattern` — used as the anchor.
    pub(crate) namespaced: String,
    /// Default severity of the lint as declared in `declare_tool_lint!`.
    pub(crate) level: Level,
    /// The third positional argument to `declare_tool_lint!`.
    pub(crate) short_desc: String,
    /// The concatenated `///` doc comment lines, in markdown form.
    pub(crate) doc_markdown: String,
    /// Source path relative to the repo root, for cross-linking.
    pub(crate) relative_source: PathBuf,
    /// `Config` struct contents when the rule declares one. `None`
    /// means the rule file has no `Config` / `CONFIG_KEY` pair.
    pub(crate) config: Option<ConfigDoc>,
}

/// The configuration surface of a single rule, as extracted from the
/// rule's `Config` struct paired with its `CONFIG_KEY` constant.
#[derive(Clone)]
pub(crate) struct ConfigDoc {
    /// The TOML table key, e.g. `perfectionist::flat_module_pattern`.
    /// Read from the file's `CONFIG_KEY` constant verbatim.
    pub(crate) key: String,
    /// One entry per named field of the `Config` struct.
    pub(crate) fields: Vec<ConfigField>,
    /// Non-built-in types referenced from `Config`'s fields, in the
    /// order the reader is most likely to want them: every type that
    /// appears earlier in the field list comes first.
    pub(crate) custom_types: Vec<TypeDoc>,
}

/// One configurable knob. Each rule's `Config` derives
/// `#[serde(default)]`, so every field is optional in TOML; the
/// renderer marks them with an `optional` badge rather than
/// reproducing the Rust default expression — the prose doc comment
/// states the default in human-readable form.
#[derive(Clone)]
pub(crate) struct ConfigField {
    /// The TOML key a reader writes in `dylint.toml`. Resolved in
    /// this order: per-field `#[serde(rename = "...")]` if present;
    /// otherwise the struct-level `#[serde(rename_all = "...")]`
    /// applied to the Rust identifier; otherwise the raw Rust
    /// identifier (the project convention has Configs in
    /// `snake_case` with `rename_all = "snake_case"`, which makes
    /// the Rust ident and the TOML key coincide).
    pub(crate) name: String,
    /// TOML-flavoured label for the field's type (e.g. `[string]`
    /// for `Vec<char>`). See `toml_type_label` for the mapping
    /// rules; the source-level type is intentionally not surfaced,
    /// since TOML authors don't write Rust syntax in `dylint.toml`.
    pub(crate) type_label: String,
    /// Per-field `///` doc comment, in markdown form.
    pub(crate) doc_markdown: String,
}

/// A non-built-in type that one of the `Config` fields references.
/// Renders as a sub-block under the rule's Configuration section so
/// readers know what shape an enum variant or nested struct takes
/// in TOML without leaving the page.
#[derive(Clone)]
pub(crate) struct TypeDoc {
    /// Rust identifier of the type (e.g. `Scope`).
    pub(crate) name: String,
    /// Doc comment attached to the `enum` or `struct` definition.
    pub(crate) doc_markdown: String,
    /// Whether the type is an enum or a struct, and the per-variant
    /// or per-field detail that comes with it.
    pub(crate) kind: TypeKind,
}

#[derive(Clone)]
pub(crate) enum TypeKind {
    Enum { variants: Vec<EnumVariant> },
    Struct { fields: Vec<StructField> },
}

#[derive(Clone)]
pub(crate) struct EnumVariant {
    /// Rust identifier, e.g. `Line`.
    pub(crate) rust_name: String,
    /// The string a TOML author would write — typically
    /// `rust_name` lowercased and snake-cased when the enum carries
    /// `#[serde(rename_all = "snake_case")]`.
    pub(crate) serialized: String,
    pub(crate) doc_markdown: String,
}

/// A field of a project-local struct surfaced under a rule's
/// Types section. Mirrors [`ConfigField`] for the nested-struct
/// case, but the renderer omits the optional badge here — a
/// custom struct's TOML representation requires every field
/// to be present, since serde's `default` attribute belongs to
/// the top-level `Config` deserialisation, not nested.
#[derive(Clone)]
pub(crate) struct StructField {
    /// The TOML key for this field, after applying the struct's
    /// own serde rename rules where present.
    pub(crate) name: String,
    /// TOML-flavoured label for the field's type. See
    /// `toml_type_label` for the mapping.
    pub(crate) type_label: String,
    /// Per-field `///` doc comment, in markdown form.
    pub(crate) doc_markdown: String,
}

/// The set of lint levels rustc / Dylint accept as the second
/// positional argument to `declare_tool_lint!`. An unrecognised
/// identifier here is a hard error rather than a silent fallback, so
/// future additions to rustc's level list are caught immediately.
#[derive(Debug, Clone, Copy, EnumString, Display)]
pub(crate) enum Level {
    Warn,
    Deny,
    Forbid,
    Allow,
}

impl Level {
    pub(crate) fn css_class(self) -> &'static str {
        match self {
            Level::Warn => "level-warn",
            Level::Deny | Level::Forbid => "level-deny",
            Level::Allow => "level-allow",
        }
    }
}
