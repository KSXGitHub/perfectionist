//! Generate the static documentation site for perfectionist's
//! implemented lints.
//!
//! Reads each `src/rules/*.rs` (skipping `mod.rs`-style index files
//! that don't declare a lint), locates the `declare_tool_lint!`
//! invocation, and pulls four things out of it: the lint identifier
//! (`pub perfectionist::NAME`), its default level, its one-line
//! description, and the `///` doc-comment block that documents it.
//! In addition, when a rule's file defines a `Config` struct paired
//! with a `CONFIG_KEY` constant, the configurable fields and any
//! non-built-in types they reference (enums, project-local structs)
//! are surfaced too. The output is a single self-contained
//! `index.html` written into the directory passed on the command
//! line.
//!
//! The macro grammar is fixed by `rustc_session::declare_tool_lint!`
//! and the project's convention of placing the doc comment inside
//! the macro braces, so a hand-rolled `syn::parse::Parse` impl is
//! enough — we don't need to invoke the dylint driver or stand up a
//! rustc plugin host just to read these few fields.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::LazyLock,
};

use cargo_toml::{Inheritable, Manifest};
use clap::Parser;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use proc_macro2::TokenStream;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Tag, TagEnd, html as cmark_html};
use quote::ToTokens;
use strum::{Display, EnumString};
use syn::{
    Attribute, Expr, ExprLit, Ident, Item, Lit, LitStr, Meta, Token, Type,
    parse::{Parse, ParseStream},
};
use syntect::{
    highlighting::{Theme, ThemeSet},
    html::{ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

#[derive(Parser)]
#[clap(about = "Render perfectionist's lint catalogue to a static HTML page")]
struct Cli {
    #[clap(help = "Repository root containing Cargo.toml and src/rules/")]
    root: PathBuf,

    #[clap(help = "Output directory; index.html will be written here")]
    out_dir: PathBuf,

    /// Git ref the rendered "Source:" links should target.
    /// Defaults to `master` so the dev-snapshot page keeps linking
    /// to tip-of-tree. CI for a tagged release should pass the
    /// tag (or its SHA) so the published page points at the
    /// source the page was actually generated from.
    #[clap(long, default_value = "master")]
    git_ref: String,
}

fn main() -> ExitCode {
    let Cli {
        root,
        out_dir,
        git_ref,
    } = Cli::parse();

    let manifest = Manifest::from_path(root.join("Cargo.toml")).expect("failed to read Cargo.toml");
    let crate_version = manifest
        .package
        .as_ref()
        .and_then(|package| match &package.version {
            Inheritable::Set(value) => Some(value.clone()),
            Inheritable::Inherited => None,
        })
        .unwrap_or_else(|| "unknown".to_owned());

    let rules_dir = root.join("src").join("rules");
    let mut rules = collect_rules(&rules_dir);
    rules.sort_by(|a, b| a.namespaced.cmp(&b.namespaced));

    if rules.is_empty() {
        eprintln!("no rules found under {}", rules_dir.display());
        return ExitCode::FAILURE;
    }

    fs::create_dir_all(&out_dir).expect("failed to create output directory");
    let html = render_page(&rules, &crate_version, &git_ref);
    let index_path = out_dir.join("index.html");
    fs::write(&index_path, html).expect("failed to write index.html");

    eprintln!("wrote {} rule(s) to {}", rules.len(), index_path.display());
    ExitCode::SUCCESS
}

/// One lint, in the shape the page needs to render.
struct Rule {
    /// `perfectionist::flat_module_pattern` — used as the anchor.
    namespaced: String,
    /// Default severity of the lint as declared in `declare_tool_lint!`.
    level: Level,
    /// The third positional argument to `declare_tool_lint!`.
    short_desc: String,
    /// The concatenated `///` doc comment lines, in markdown form.
    doc_markdown: String,
    /// Source path relative to the repo root, for cross-linking.
    relative_source: PathBuf,
    /// `Config` struct contents when the rule declares one. `None`
    /// means the rule file has no `Config` / `CONFIG_KEY` pair.
    config: Option<ConfigDoc>,
}

/// The configuration surface of a single rule, as extracted from the
/// rule's `Config` struct paired with its `CONFIG_KEY` constant.
struct ConfigDoc {
    /// The TOML table key, e.g. `perfectionist::flat_module_pattern`.
    /// Read from the file's `CONFIG_KEY` constant verbatim.
    key: String,
    /// One entry per named field of the `Config` struct.
    fields: Vec<ConfigField>,
    /// Non-built-in types referenced from `Config`'s fields, in the
    /// order the reader is most likely to want them: every type that
    /// appears earlier in the field list comes first.
    custom_types: Vec<TypeDoc>,
}

/// One configurable knob. Each rule's `Config` derives
/// `#[serde(default)]`, so every field is optional in TOML; the
/// renderer marks them with an `optional` badge rather than
/// reproducing the Rust default expression — the prose doc comment
/// states the default in human-readable form.
struct ConfigField {
    /// The TOML key a reader writes in `dylint.toml`. Resolved in
    /// this order: per-field `#[serde(rename = "...")]` if present;
    /// otherwise the struct-level `#[serde(rename_all = "...")]`
    /// applied to the Rust identifier; otherwise the raw Rust
    /// identifier (the project convention has Configs in
    /// `snake_case` with `rename_all = "snake_case"`, which makes
    /// the Rust ident and the TOML key coincide).
    name: String,
    /// TOML-flavoured label for the field's type (e.g. `[string]`
    /// for `Vec<char>`). See [`toml_type_label`] for the mapping
    /// rules; the source-level type is intentionally not surfaced,
    /// since TOML authors don't write Rust syntax in `dylint.toml`.
    type_label: String,
    /// Per-field `///` doc comment, in markdown form.
    doc_markdown: String,
}

/// A non-built-in type that one of the `Config` fields references.
/// Renders as a sub-block under the rule's Configuration section so
/// readers know what shape an enum variant or nested struct takes
/// in TOML without leaving the page.
struct TypeDoc {
    /// Rust identifier of the type (e.g. `Scope`).
    name: String,
    /// Doc comment attached to the `enum` or `struct` definition.
    doc_markdown: String,
    /// Whether the type is an enum or a struct, and the per-variant
    /// or per-field detail that comes with it.
    kind: TypeKind,
}

enum TypeKind {
    Enum { variants: Vec<EnumVariant> },
    Struct { fields: Vec<StructField> },
}

struct EnumVariant {
    /// Rust identifier, e.g. `Line`.
    rust_name: String,
    /// The string a TOML author would write — typically
    /// `rust_name` lowercased and snake-cased when the enum carries
    /// `#[serde(rename_all = "snake_case")]`.
    serialized: String,
    doc_markdown: String,
}

/// A field of a project-local struct surfaced under a rule's
/// Types section. Mirrors [`ConfigField`] for the nested-struct
/// case, but the renderer omits the optional badge here — a
/// custom struct's TOML representation requires every field
/// to be present, since serde's `default` attribute belongs to
/// the top-level `Config` deserialisation, not nested.
struct StructField {
    /// The TOML key for this field, after applying the struct's
    /// own serde rename rules where present.
    name: String,
    /// TOML-flavoured label for the field's type. See
    /// [`toml_type_label`] for the mapping.
    type_label: String,
    /// Per-field `///` doc comment, in markdown form.
    doc_markdown: String,
}

/// The set of lint levels rustc / Dylint accept as the second
/// positional argument to `declare_tool_lint!`. An unrecognised
/// identifier here is a hard error rather than a silent fallback, so
/// future additions to rustc's level list are caught immediately.
#[derive(Debug, Clone, Copy, EnumString, Display)]
enum Level {
    Warn,
    Deny,
    Forbid,
    Allow,
}

impl Level {
    fn css_class(self) -> &'static str {
        match self {
            Level::Warn => "level-warn",
            Level::Deny | Level::Forbid => "level-deny",
            Level::Allow => "level-allow",
        }
    }
}

fn collect_rules(rules_dir: &Path) -> Vec<Rule> {
    let entries = fs::read_dir(rules_dir).expect("failed to read src/rules/");
    let mut rules = Vec::new();
    for entry in entries {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let Some(rule) = extract_rule(&path) else {
            continue;
        };
        rules.push(rule);
    }
    rules
}

fn extract_rule(source_path: &Path) -> Option<Rule> {
    let source = fs::read_to_string(source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
    let file = syn::parse_file(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", source_path.display()));
    let macro_item = file.items.iter().find_map(|item| match item {
        Item::Macro(item_macro)
            if item_macro
                .mac
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "declare_tool_lint") =>
        {
            Some(&item_macro.mac.tokens)
        }
        _ => None,
    })?;

    let declaration = syn::parse2::<DeclareToolLint>(macro_item.clone()).unwrap_or_else(|error| {
        panic!(
            "failed to parse declare_tool_lint! body in {}: {error}",
            source_path.display()
        )
    });

    let namespaced = format!(
        "perfectionist::{}",
        declaration.name.to_string().to_ascii_lowercase()
    );
    let doc_markdown = doc_attrs_to_markdown(&declaration.attrs);
    let relative_source = source_path
        .components()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let level_ident = declaration.level.to_string();
    let level: Level = level_ident.parse().unwrap_or_else(|_| {
        panic!(
            "unknown lint level `{level_ident}` in {}",
            source_path.display()
        )
    });

    let config = extract_config(source_path, &file);

    Some(Rule {
        namespaced,
        level,
        short_desc: declaration.desc.value(),
        doc_markdown,
        relative_source,
        config,
    })
}

/// Locate the rule's `Config` struct and its `CONFIG_KEY` constant
/// and bundle them — along with any project-local types the fields
/// reference — into a `ConfigDoc`. Returns `None` when *both* the
/// constant and the struct are missing (a rule with no configuration
/// concept at all). Defining only one of the two prints a warning
/// and still returns `None`: the convention in this repo is that
/// every rule has both, and a half-defined Config is almost always
/// the author having dropped the other half.
fn extract_config(source_path: &Path, file: &syn::File) -> Option<ConfigDoc> {
    let key = file.items.iter().find_map(|item| match item {
        Item::Const(item_const) if item_const.ident == "CONFIG_KEY" => match &*item_const.expr {
            Expr::Lit(ExprLit {
                lit: Lit::Str(literal),
                ..
            }) => Some(literal.value()),
            _ => None,
        },
        _ => None,
    });
    let config_struct = file.items.iter().find_map(|item| match item {
        Item::Struct(item_struct) if item_struct.ident == "Config" => Some(item_struct),
        _ => None,
    });
    let (key, config_struct) = match (key, config_struct) {
        (Some(key), Some(config_struct)) => (key, config_struct),
        (None, None) => return None,
        (Some(_), None) => {
            eprintln!(
                "warning: {} declares CONFIG_KEY but no `Config` struct; \
                 skipping its configuration section",
                source_path.display(),
            );
            return None;
        }
        (None, Some(_)) => {
            eprintln!(
                "warning: {} declares a `Config` struct but no CONFIG_KEY const; \
                 skipping its configuration section",
                source_path.display(),
            );
            return None;
        }
    };
    let rename_all = serde_str_attr(&config_struct.attrs, "rename_all");

    let named_fields = match &config_struct.fields {
        syn::Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    let fields = named_fields
        .iter()
        .map(|field| {
            let rust_name = field
                .ident
                .as_ref()
                .expect("named field always has an ident")
                .to_string();
            // Precedence mirrors serde itself: per-field
            // `#[serde(rename = "...")]` wins, then the struct-
            // level `rename_all` style, then the raw Rust ident.
            let name = serde_str_attr(&field.attrs, "rename").unwrap_or_else(|| {
                rename_all
                    .as_deref()
                    .map(|style| apply_rename_all(style, &rust_name))
                    .unwrap_or(rust_name)
            });
            ConfigField {
                name,
                type_label: toml_type_label(&field.ty),
                doc_markdown: doc_attrs_to_markdown(&field.attrs),
            }
        })
        .collect();

    let mut referenced = Vec::new();
    let mut seen = BTreeSet::new();
    for field in &named_fields {
        collect_referenced_idents(&field.ty, &mut referenced, &mut seen);
    }
    let custom_types = referenced
        .into_iter()
        .filter_map(|ident| find_type_doc(file, &ident))
        .collect();

    Some(ConfigDoc {
        key,
        fields,
        custom_types,
    })
}

/// Walk a `syn::Type` and collect every type identifier that isn't a
/// well-known built-in. Only the *last* segment of each path is
/// considered, matching [`toml_type_label`]'s rule: leading
/// segments like `std::vec` in `std::vec::Vec<T>` are noise (the
/// generator only looks up types against the rule file's local
/// items, so qualified paths would never resolve anyway).
/// Insertion order matches the order each distinct ident is first
/// encountered, so the rendered docs list types in the order a
/// reader scanning the field list will meet them. The `seen` set
/// deduplicates across multiple fields.
fn collect_referenced_idents(ty: &Type, out: &mut Vec<String>, seen: &mut BTreeSet<String>) {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                let name = segment.ident.to_string();
                if !is_builtin_type(&name) && seen.insert(name.clone()) {
                    out.push(name);
                }
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(inner) = arg {
                            collect_referenced_idents(inner, out, seen);
                        }
                    }
                }
            }
        }
        Type::Reference(type_ref) => collect_referenced_idents(&type_ref.elem, out, seen),
        Type::Tuple(type_tuple) => {
            for elem in &type_tuple.elems {
                collect_referenced_idents(elem, out, seen);
            }
        }
        Type::Array(type_array) => collect_referenced_idents(&type_array.elem, out, seen),
        Type::Slice(type_slice) => collect_referenced_idents(&type_slice.elem, out, seen),
        Type::Paren(type_paren) => collect_referenced_idents(&type_paren.elem, out, seen),
        Type::Group(type_group) => collect_referenced_idents(&type_group.elem, out, seen),
        _ => {}
    }
}

/// Whether `name` is in the renderer's built-in type set. Thin
/// wrapper around [`BUILTIN_TYPES`] kept for naming intent at the
/// call sites; the contract and rationale live on the constant.
fn is_builtin_type(name: &str) -> bool {
    BUILTIN_TYPES.contains(&name)
}

/// Type names the renderer treats as "obvious", omitting them from
/// the per-rule custom-types listing. Covers Rust's primitives and
/// the std-library containers that show up in serde-deserialised
/// configuration values. `Config` itself is on the list so a
/// self-referential type doesn't loop the lookup.
///
/// **Keep in sync with [`toml_type_label`].** The two share a
/// coverage contract: every ident here must also have a match arm
/// there, otherwise the renderer either drops a real custom type
/// from the Types section (false positive here) or leaks a Rust
/// identifier into the field-type column (false negative there).
/// The unit test `builtin_types_all_map_to_toml_label` guards this
/// contract mechanically.
const BUILTIN_TYPES: &[&str] = &[
    // Primitives.
    "bool",
    "char",
    "str",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "usize",
    "isize",
    "f32",
    "f64",
    // Common std types likely to appear in serde-deserialised configs.
    "String",
    "Vec",
    "Option",
    "Box",
    "Rc",
    "Arc",
    "Cow",
    "PathBuf",
    "Path",
    "OsString",
    "OsStr",
    "HashMap",
    "HashSet",
    "BTreeMap",
    "BTreeSet",
    "VecDeque",
    "LinkedList",
    // The Config struct itself.
    "Config",
];

/// Find the `enum` or `struct` definition for `ident` inside `file`
/// and produce a `TypeDoc` describing its variants or fields.
/// Returns `None` for idents we can't locate (e.g., types imported
/// from another crate); those are silently dropped rather than
/// faked, since the docs are only useful when we can show the real
/// shape.
fn find_type_doc(file: &syn::File, ident: &str) -> Option<TypeDoc> {
    for item in &file.items {
        match item {
            Item::Enum(item_enum) if item_enum.ident == ident => {
                let rename_all = serde_str_attr(&item_enum.attrs, "rename_all");
                let variants = item_enum
                    .variants
                    .iter()
                    .map(|variant| {
                        let rust_name = variant.ident.to_string();
                        let variant_rename = serde_str_attr(&variant.attrs, "rename");
                        let serialized = variant_rename
                            .or_else(|| {
                                rename_all
                                    .as_deref()
                                    .map(|style| apply_rename_all(style, &rust_name))
                            })
                            .unwrap_or_else(|| rust_name.clone());
                        EnumVariant {
                            rust_name,
                            serialized,
                            doc_markdown: doc_attrs_to_markdown(&variant.attrs),
                        }
                    })
                    .collect();
                return Some(TypeDoc {
                    name: ident.to_owned(),
                    doc_markdown: doc_attrs_to_markdown(&item_enum.attrs),
                    kind: TypeKind::Enum { variants },
                });
            }
            Item::Struct(item_struct) if item_struct.ident == ident => {
                let fields = match &item_struct.fields {
                    syn::Fields::Named(named) => named
                        .named
                        .iter()
                        .map(|field| StructField {
                            name: field
                                .ident
                                .as_ref()
                                .expect("named field always has an ident")
                                .to_string(),
                            type_label: toml_type_label(&field.ty),
                            doc_markdown: doc_attrs_to_markdown(&field.attrs),
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                return Some(TypeDoc {
                    name: ident.to_owned(),
                    doc_markdown: doc_attrs_to_markdown(&item_struct.attrs),
                    kind: TypeKind::Struct { fields },
                });
            }
            _ => {}
        }
    }
    None
}

/// Read a `#[serde(key = "literal")]` string value, scanning every
/// `serde(...)` attribute on the item. Returns the first match in
/// source order; an attribute whose body fails to parse is silently
/// skipped, since this generator runs against rule sources that the
/// compiler has already accepted — a parse failure here means the
/// attribute uses a form (e.g. `serde(deny_unknown_fields)`) that
/// the generator simply doesn't surface.
fn serde_str_attr(attrs: &[Attribute], key: &str) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let parsed =
            attr.parse_args_with(syn::punctuated::Punctuated::<Meta, Token![,]>::parse_terminated);
        let Ok(items) = parsed else { continue };
        for item in items {
            if let Meta::NameValue(name_value) = item
                && name_value.path.is_ident(key)
                && let Expr::Lit(ExprLit {
                    lit: Lit::Str(literal),
                    ..
                }) = name_value.value
            {
                return Some(literal.value());
            }
        }
    }
    None
}

/// Apply one of serde's `rename_all` styles to a Rust identifier.
/// Covers the styles serde itself documents: `snake_case`,
/// `kebab-case`, their SCREAMING variants, `lowercase`, `UPPERCASE`,
/// `camelCase`, and `PascalCase`. None of the current rules need
/// anything outside `snake_case`; the others are here so that
/// adopting a new style elsewhere doesn't silently mangle the
/// rendered TOML keys.
///
/// An unrecognised style prints a warning to stderr and falls
/// through to the identifier as written. Warning rather than
/// panicking, on the principle that a wrong doc page is less bad
/// than a broken doc-generation build — but the warning makes the
/// gap loud enough that the rule author notices.
fn apply_rename_all(style: &str, name: &str) -> String {
    match style {
        "snake_case" => pascal_to_snake(name),
        "SCREAMING_SNAKE_CASE" => pascal_to_snake(name).to_ascii_uppercase(),
        "kebab-case" => pascal_to_snake(name).replace('_', "-"),
        "SCREAMING-KEBAB-CASE" => pascal_to_snake(name).replace('_', "-").to_ascii_uppercase(),
        "lowercase" => name.to_ascii_lowercase(),
        "UPPERCASE" => name.to_ascii_uppercase(),
        // Rust enum variants are already `PascalCase` by convention.
        "PascalCase" => name.to_owned(),
        // `camelCase` is `PascalCase` with the first character
        // lower-cased; everything else stays as written.
        "camelCase" => {
            let mut chars = name.chars();
            match chars.next() {
                Some(first) => first.to_lowercase().chain(chars).collect(),
                None => String::new(),
            }
        }
        unknown => {
            eprintln!(
                "warning: unrecognised serde `rename_all` style {unknown:?}; \
                 rendering `{name}` unchanged. Add an arm to `apply_rename_all` \
                 if this style needs to be supported."
            );
            name.to_owned()
        }
    }
}

/// Convert `PascalCase` (or `camelCase`) to `snake_case` by inserting
/// `_` before each uppercase letter that follows a lowercase one or
/// precedes a lowercase one. Adequate for the rule-author-controlled
/// enum names this generator sees; not a general-purpose case
/// converter.
fn pascal_to_snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 2);
    let chars: Vec<char> = name.chars().collect();
    for (index, &ch) in chars.iter().enumerate() {
        if ch.is_ascii_uppercase() {
            let prev_lower = index > 0 && chars[index - 1].is_ascii_lowercase();
            let next_lower = chars
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_lowercase());
            if index > 0 && (prev_lower || next_lower) {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Translate a `syn::Type` into a TOML-flavoured type label. The
/// renderer never shows Rust syntax to the reader; TOML authors
/// write arrays as `[a, b]`, integers without sign annotations, and
/// strings without `char` / `&str` / `Cow` distinctions, so the
/// labels echo that vocabulary. The translation is purely structural
/// — no ad-hoc exceptions for specific field names — so adding a
/// new built-in type means extending the match arms here.
///
/// - `bool` → `boolean`
/// - `u*` / `usize` → `unsigned integer`
/// - `i*` / `isize` → `integer`
/// - `f32` / `f64` → `float`
/// - `String` / `&str` / `char` / `OsString` / `PathBuf` / `Cow<…>` → `string`
/// - `Vec<T>` / `HashSet<T>` / `BTreeSet<T>` / `VecDeque<T>` /
///   `LinkedList<T>` → `[label-of-T]`
/// - `HashMap<_, V>` / `BTreeMap<_, V>` → `table of label-of-V`
/// - `Option<T>` → `label-of-T` (every config field is already
///   marked optional, so the wrapper would just add noise)
/// - `Option<T>` / `Box<T>` / `Rc<T>` / `Arc<T>` → `label-of-T`
///   (every config field is already marked optional, and the
///   smart-pointer wrappers are transparent at the serde layer)
/// - Anything else (project-local enums and structs) → the Rust
///   identifier verbatim, since those names appear in the per-rule
///   Types subsection below and the reader can scan to them.
///
/// **Keep in sync with [`is_builtin_type`].** Both functions must
/// recognise the same set of identifiers; see the note on
/// `is_builtin_type` for why.
fn toml_type_label(ty: &Type) -> String {
    match ty {
        Type::Path(type_path) => {
            let Some(segment) = type_path.path.segments.last() else {
                return ty.to_token_stream().to_string();
            };
            let ident = segment.ident.to_string();
            let inner_types: Vec<&Type> = match &segment.arguments {
                syn::PathArguments::AngleBracketed(args) => args
                    .args
                    .iter()
                    .filter_map(|arg| match arg {
                        syn::GenericArgument::Type(inner) => Some(inner),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            match ident.as_str() {
                "bool" => "boolean".to_owned(),
                "String" | "str" | "char" | "OsString" | "OsStr" | "Path" | "PathBuf" | "Cow" => {
                    "string".to_owned()
                }
                "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => "unsigned integer".to_owned(),
                "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => "integer".to_owned(),
                "f32" | "f64" => "float".to_owned(),
                "Vec" | "HashSet" | "BTreeSet" | "VecDeque" | "LinkedList" => {
                    match inner_types.first() {
                        Some(inner) => format!("[{}]", toml_type_label(inner)),
                        None => "array".to_owned(),
                    }
                }
                "HashMap" | "BTreeMap" => match inner_types.get(1) {
                    Some(value) => format!("table of {}", toml_type_label(value)),
                    None => "table".to_owned(),
                },
                "Option" | "Box" | "Rc" | "Arc" => match inner_types.first() {
                    Some(inner) => toml_type_label(inner),
                    None => ident,
                },
                _ => ident,
            }
        }
        Type::Reference(type_ref) => toml_type_label(&type_ref.elem),
        Type::Paren(type_paren) => toml_type_label(&type_paren.elem),
        Type::Group(type_group) => toml_type_label(&type_group.elem),
        // Tuples, trait objects, function pointers, raw pointers,
        // etc. fall through to a Rust-syntax fallback. No current
        // `Config` field uses any of these; if one ever does, add
        // a real arm rather than letting Rust syntax leak into the
        // user-facing label.
        _ => ty.to_token_stream().to_string(),
    }
}

/// Minimal grammar of `declare_tool_lint!`'s body. The macro itself
/// allows arbitrary `key: value` pairs after the description; we
/// don't currently surface them on the page, so the trailing tokens
/// are accepted and discarded.
struct DeclareToolLint {
    attrs: Vec<Attribute>,
    name: Ident,
    level: Ident,
    desc: LitStr,
}

impl Parse for DeclareToolLint {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let _vis: Token![pub] = input.parse()?;
        let _tool: Ident = input.parse()?;
        let _colon: Token![::] = input.parse()?;
        let name: Ident = input.parse()?;
        let _comma1: Token![,] = input.parse()?;
        let level: Ident = input.parse()?;
        let _comma2: Token![,] = input.parse()?;
        let desc: LitStr = input.parse()?;
        let _rest: TokenStream = input.parse()?;
        Ok(Self {
            attrs,
            name,
            level,
            desc,
        })
    }
}

fn doc_attrs_to_markdown(attrs: &[Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let Meta::NameValue(meta) = &attr.meta else {
            continue;
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) = &meta.value
        else {
            continue;
        };
        // `/// foo` lexes as `#[doc = " foo"]`; drop the single
        // convention-space so the markdown round-trips cleanly.
        let raw = s.value();
        let trimmed = raw.strip_prefix(' ').unwrap_or(&raw).to_owned();
        lines.push(trimmed);
    }
    lines.join("\n")
}

fn render_page(rules: &[Rule], crate_version: &str, git_ref: &str) -> String {
    let markup: Markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "perfectionist lints" }
                style { (PreEscaped(STYLE)) (PreEscaped(&*HIGHLIGHT_CSS)) }
            }
            body {
                h1 { "perfectionist lints" }
                div.banner {
                    @if git_ref == "master" {
                        "Showing development docs from " code { "master" } ". "
                        "Latest released version: " code { (crate_version) } "."
                    } @else {
                        "Showing docs for " code { (git_ref) } "."
                    }
                }
                p {
                    "perfectionist is a Dylint plugin; see the "
                    a href="https://github.com/KSXGitHub/perfectionist" { "README" }
                    " for setup. Lint-control attributes use the "
                    code { "perfectionist::" } " namespace."
                }
                h2 { "Index" }
                table.index {
                    thead {
                        tr {
                            th { "Lint" }
                            th { "Default" }
                            th { "Description" }
                        }
                    }
                    tbody {
                        @for rule in rules {
                            tr {
                                td {
                                    a href={ "#" (anchor_for(&rule.namespaced)) } {
                                        code { (rule.namespaced) }
                                    }
                                }
                                td { (level_badge(rule.level)) }
                                td {
                                    (PreEscaped(markdown_inline_to_html(&rule.short_desc)))
                                }
                            }
                        }
                    }
                }
                h2 { "Rules" }
                @for rule in rules {
                    (rule_article(rule, git_ref))
                }
                footer {
                    "Generated from " code { "src/rules/" }
                    " at " code { (git_ref) } "."
                }
            }
        }
    };
    markup.into_string()
}

fn rule_article(rule: &Rule, git_ref: &str) -> Markup {
    let source_path = rule.relative_source.display().to_string();
    let source_url =
        format!("https://github.com/KSXGitHub/perfectionist/blob/{git_ref}/{source_path}");
    html! {
        article.rule id=(anchor_for(&rule.namespaced)) {
            h2 { code { (rule.namespaced) } }
            p {
                (level_badge(rule.level))
                (PreEscaped(markdown_inline_to_html(&rule.short_desc)))
            }
            (PreEscaped(markdown_to_html(&rule.doc_markdown)))
            @if let Some(config) = &rule.config {
                (config_section(config))
            }
            p.source {
                "Source: "
                a href=(source_url) { code { (source_path) } }
            }
        }
    }
}

/// Render the per-rule configuration block. Two shapes:
///
/// 1. When the rule has at least one configurable field, wrap the
///    block in a `<details>` element, collapsed by default. The
///    rule's description above is what most catalogue readers came
///    for; configuration is reference material that can stay
///    hidden until clicked. Modern browsers auto-expand a closed
///    `<details>` when find-in-page hits something inside it, so
///    search still works.
/// 2. When the rule has no fields (an empty `Config` struct,
///    e.g. `flat_module_pattern`), render a single inline line
///    "Configuration: none." instead. The fact that the rule is
///    not configurable is itself information; omitting the section
///    entirely would leave readers wondering whether they missed a
///    knob. Any struct-level doc comment is *not* surfaced in this
///    case — an empty config has no user-facing schema to
///    annotate, so the comment is internal implementation prose
///    that belongs in the source, not on the catalogue page.
fn config_section(config: &ConfigDoc) -> Markup {
    if config.fields.is_empty() {
        return html! {
            p.config-none {
                strong { "Configuration:" } " none."
            }
        };
    }
    html! {
        details.config-details {
            summary.config-summary { "Configuration" }
            p {
                "Configure via " code { "dylint.toml" } " under "
                code { "[\"" (config.key) "\"]" } "."
            }
            dl.config {
                @for field in &config.fields {
                    dt {
                        code.config-key { (field.name) }
                        " : "
                        code.config-type { (field.type_label) }
                        " "
                        span.badge.badge-optional { "optional" }
                    }
                    dd {
                        @if field.doc_markdown.is_empty() {
                            p { em { "Undocumented." } }
                        } @else {
                            (PreEscaped(markdown_to_html(&field.doc_markdown)))
                        }
                    }
                }
            }
            @if !config.custom_types.is_empty() {
                h4 { "Types" }
                @for ty in &config.custom_types {
                    (custom_type_block(ty))
                }
            }
        }
    }
}

/// Render one custom type referenced by a `Config` field. Enum
/// variants are listed with the string a TOML author would write,
/// since that's the user-facing value; the Rust identifier is shown
/// in parentheses only when it differs.
fn custom_type_block(ty: &TypeDoc) -> Markup {
    let kind_label = match ty.kind {
        TypeKind::Enum { .. } => "enum",
        TypeKind::Struct { .. } => "struct",
    };
    html! {
        div.custom-type {
            p {
                code.custom-type-name { (ty.name) }
                " "
                span.custom-type-kind { (kind_label) }
            }
            @if !ty.doc_markdown.is_empty() {
                (PreEscaped(markdown_to_html(&ty.doc_markdown)))
            }
            @match &ty.kind {
                TypeKind::Enum { variants } => {
                    @if !variants.is_empty() {
                        dl.config {
                            @for variant in variants {
                                dt {
                                    code.config-key { "\"" (variant.serialized) "\"" }
                                    @if variant.rust_name != variant.serialized {
                                        " "
                                        span.config-default {
                                            "(Rust: " code { (variant.rust_name) } ")"
                                        }
                                    }
                                }
                                dd {
                                    @if variant.doc_markdown.is_empty() {
                                        p { em { "Undocumented." } }
                                    } @else {
                                        (PreEscaped(markdown_to_html(&variant.doc_markdown)))
                                    }
                                }
                            }
                        }
                    }
                }
                TypeKind::Struct { fields } => {
                    @if !fields.is_empty() {
                        dl.config {
                            @for field in fields {
                                dt {
                                    code.config-key { (field.name) }
                                    " : "
                                    code.config-type { (field.type_label) }
                                }
                                dd {
                                    @if field.doc_markdown.is_empty() {
                                        p { em { "Undocumented." } }
                                    } @else {
                                        (PreEscaped(markdown_to_html(&field.doc_markdown)))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn level_badge(level: Level) -> Markup {
    let class = format!("level {}", level.css_class());
    html! {
        span class=(class) { (level.to_string()) }
    }
}

fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = pulldown_cmark::Parser::new_ext(markdown, options);
    let events = highlight_code_blocks(parser);
    let mut buffer = String::new();
    cmark_html::push_html(&mut buffer, events.into_iter());
    buffer
}

/// Walk pulldown-cmark events, replacing fenced code blocks with
/// pre-highlighted HTML produced by `syntect`. The first comma-
/// separated token of the fence's info string is the language tag
/// (so `rust,ignore` is treated as Rust). Untagged fences pass
/// through to pulldown-cmark's default rendering; unknown tags are
/// highlighted as plain text.
fn highlight_code_blocks<'a>(parser: impl Iterator<Item = Event<'a>>) -> Vec<Event<'a>> {
    let mut out = Vec::new();
    let mut current_lang: Option<String> = None;
    let mut code_buffer = String::new();
    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                let lang = info
                    .split(',')
                    .next()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned);
                if let Some(lang) = lang {
                    current_lang = Some(lang);
                    code_buffer.clear();
                } else {
                    // Untagged fenced blocks have no language to highlight;
                    // let pulldown-cmark emit them as plain `<pre><code>`.
                    out.push(Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))));
                }
            }
            Event::End(TagEnd::CodeBlock) if current_lang.is_some() => {
                let lang = current_lang.take().expect("guarded above");
                let html = highlight_to_html(&code_buffer, &lang);
                out.push(Event::Html(CowStr::Boxed(html.into_boxed_str())));
            }
            Event::Text(text) if current_lang.is_some() => {
                code_buffer.push_str(&text);
            }
            other => out.push(other),
        }
    }
    out
}

fn highlight_to_html(code: &str, lang: &str) -> String {
    let syntax_set: &SyntaxSet = &SYNTAX_SET;
    let extension = match lang {
        "rust" => "rs",
        other => other,
    };
    let syntax = syntax_set
        .find_syntax_by_extension(extension)
        .or_else(|| syntax_set.find_syntax_by_name(lang))
        .or_else(|| syntax_set.find_syntax_by_token(lang))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let mut generator =
        ClassedHTMLGenerator::new_with_class_style(syntax, syntax_set, ClassStyle::Spaced);
    for line in LinesWithEndings::from(code) {
        // The classed generator only fails on malformed regex
        // matches, which can't happen against the bundled syntaxes.
        generator
            .parse_html_for_line_which_includes_newline(line)
            .expect("classed highlighting of bundled syntax should not fail");
    }
    let body = generator.finalize();
    let class_attr = lang_class_attr(lang);
    format!("<pre><code{class_attr}>{body}</code></pre>\n")
}

/// Render the `language-…` class attribute for a code block, but
/// only when the fence's language tag is made of characters that
/// are unambiguously safe to drop into an HTML attribute. Anything
/// outside the allow-list yields an empty attribute — the page
/// still renders correctly because the inline `<span class>`es
/// generated by `syntect` carry the actual highlighting, and the
/// `language-…` class is purely decorative here.
fn lang_class_attr(lang: &str) -> String {
    let safe = !lang.is_empty()
        && lang
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '#' | '.'));
    if safe {
        format!(" class=\"language-{lang}\"")
    } else {
        String::new()
    }
}

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME: LazyLock<Theme> = LazyLock::new(|| {
    ThemeSet::load_defaults()
        .themes
        .remove("InspiredGitHub")
        .expect("InspiredGitHub theme is bundled with syntect")
});
static HIGHLIGHT_CSS: LazyLock<String> = LazyLock::new(|| {
    css_for_theme_with_class_style(&THEME, ClassStyle::Spaced)
        .expect("generating CSS for a bundled theme should not fail")
});

/// Render a short, single-line snippet of markdown without the
/// outer `<p>…</p>` wrapper that block-level rendering inserts. Used
/// for table cells and inline headings where backticks should still
/// turn into `<code>`.
fn markdown_inline_to_html(markdown: &str) -> String {
    let parser = pulldown_cmark::Parser::new(markdown).filter(|event| {
        !matches!(
            event,
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph)
        )
    });
    let mut buffer = String::new();
    cmark_html::push_html(&mut buffer, parser);
    buffer.trim_end().to_owned()
}

fn anchor_for(namespaced: &str) -> String {
    namespaced.replace("::", "-")
}

const STYLE: &str = include_str!("style.css");

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_type(source: &str) -> Type {
        syn::parse_str(source).expect("test input should parse as a syn::Type")
    }

    /// Parse a Rust item and return its outer attributes. Used to
    /// drive `serde_str_attr` tests without standing up a full
    /// `syn::Field` or `syn::Variant` value by hand.
    fn parse_attrs(source: &str) -> Vec<Attribute> {
        let parsed: syn::ItemStruct =
            syn::parse_str(source).expect("test input should parse as an item struct");
        parsed.attrs
    }

    #[test]
    fn pascal_to_snake_basic() {
        assert_eq!(pascal_to_snake("Line"), "line");
        assert_eq!(pascal_to_snake("BlockComment"), "block_comment");
        assert_eq!(pascal_to_snake("XMLParser"), "xml_parser");
        assert_eq!(pascal_to_snake("HTTPServer"), "http_server");
        assert_eq!(pascal_to_snake("URL"), "url");
        assert_eq!(pascal_to_snake("already_snake"), "already_snake");
    }

    #[test]
    fn toml_type_label_primitives() {
        assert_eq!(toml_type_label(&parse_type("bool")), "boolean");
        assert_eq!(toml_type_label(&parse_type("usize")), "unsigned integer");
        assert_eq!(toml_type_label(&parse_type("i32")), "integer");
        assert_eq!(toml_type_label(&parse_type("f64")), "float");
        assert_eq!(toml_type_label(&parse_type("String")), "string");
        assert_eq!(toml_type_label(&parse_type("char")), "string");
    }

    #[test]
    fn toml_type_label_arrays_and_maps() {
        assert_eq!(toml_type_label(&parse_type("Vec<char>")), "[string]");
        assert_eq!(
            toml_type_label(&parse_type("Vec<Vec<u8>>")),
            "[[unsigned integer]]"
        );
        assert_eq!(
            toml_type_label(&parse_type("HashMap<String, usize>")),
            "table of unsigned integer",
        );
    }

    #[test]
    fn toml_type_label_transparent_wrappers() {
        // Option, Box, Rc, Arc unwrap to the inner type.
        assert_eq!(toml_type_label(&parse_type("Option<String>")), "string");
        assert_eq!(
            toml_type_label(&parse_type("Box<usize>")),
            "unsigned integer"
        );
        assert_eq!(toml_type_label(&parse_type("Arc<Vec<String>>")), "[string]");
    }

    #[test]
    fn toml_type_label_qualified_paths_use_last_segment() {
        assert_eq!(
            toml_type_label(&parse_type("std::vec::Vec<String>")),
            "[string]"
        );
        assert_eq!(
            toml_type_label(&parse_type("alloc::string::String")),
            "string"
        );
    }

    #[test]
    fn toml_type_label_custom_idents_pass_through() {
        // Project-local enums/structs surface verbatim; readers
        // follow them to the per-rule Types subsection.
        assert_eq!(toml_type_label(&parse_type("Scope")), "Scope");
        assert_eq!(toml_type_label(&parse_type("Vec<Scope>")), "[Scope]");
    }

    #[test]
    fn collect_referenced_idents_skips_builtins_and_keeps_order() {
        let ty = parse_type("HashMap<String, Vec<Scope>>");
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        collect_referenced_idents(&ty, &mut out, &mut seen);
        assert_eq!(out, vec!["Scope".to_owned()]);
    }

    #[test]
    fn builtin_types_all_map_to_toml_label() {
        // Coverage contract: every name in BUILTIN_TYPES must have
        // a matching arm in `toml_type_label`, otherwise the
        // renderer would drop the name from the Types section
        // (because `is_builtin_type` returns true) AND emit it
        // verbatim as a Rust identifier in the field-type column.
        //
        // The `Config` entry is exempt — it's on the builtin list
        // purely to prevent self-referential lookup loops, and has
        // no user-facing label form.
        //
        // Generic types need a concrete instantiation to exercise
        // the arm (a bare `Option` with no inner type would hit
        // the `_ => ident` fallback). `bool` is harmless filler.
        for &name in BUILTIN_TYPES {
            if name == "Config" {
                continue;
            }
            let source = match name {
                "Vec" | "HashSet" | "BTreeSet" | "VecDeque" | "LinkedList" | "Option" | "Box"
                | "Rc" | "Arc" | "Cow" => format!("{name}<bool>"),
                "HashMap" | "BTreeMap" => format!("{name}<String, bool>"),
                _ => name.to_owned(),
            };
            let label = toml_type_label(&parse_type(&source));
            assert_ne!(
                label, name,
                "builtin `{name}` is missing a `toml_type_label` arm \
                 (rendered `{source}` as `{label}`); either add one \
                 or remove the entry from BUILTIN_TYPES",
            );
        }
    }

    #[test]
    fn serde_str_attr_branches() {
        // Picks the value of the requested key.
        let attrs = parse_attrs(r#"#[serde(rename_all = "snake_case")] struct S;"#);
        assert_eq!(
            serde_str_attr(&attrs, "rename_all"),
            Some("snake_case".to_owned())
        );
        assert_eq!(serde_str_attr(&attrs, "rename"), None);

        // Ignores non-`serde` attributes.
        let attrs = parse_attrs(r#"#[derive(Debug)] #[other(rename = "x")] struct S;"#);
        assert_eq!(serde_str_attr(&attrs, "rename"), None);

        // Mixed Path / NameValue items inside `serde(...)`: the
        // path-form `default` is skipped, the name-value matches.
        let attrs = parse_attrs(r#"#[serde(default, rename = "foo")] struct S;"#);
        assert_eq!(serde_str_attr(&attrs, "rename"), Some("foo".to_owned()));

        // First match wins when the key appears more than once.
        let attrs =
            parse_attrs(r#"#[serde(rename = "first")] #[serde(rename = "second")] struct S;"#);
        assert_eq!(serde_str_attr(&attrs, "rename"), Some("first".to_owned()));

        // Non-string value is rejected (only `Lit::Str` is accepted).
        let attrs = parse_attrs(r#"#[serde(skip = true)] struct S;"#);
        assert_eq!(serde_str_attr(&attrs, "skip"), None);
    }

    #[test]
    fn config_field_honours_serde_rename() {
        // Walk a synthetic rule file through `extract_config` to
        // confirm that `#[serde(rename = "...")]` on a Config
        // field surfaces as the TOML key instead of the Rust ident.
        let source = r#"
            const CONFIG_KEY: &str = "perfectionist::demo";

            #[derive(serde::Deserialize)]
            #[serde(default, rename_all = "snake_case")]
            struct Config {
                #[serde(rename = "renamed-key")]
                rust_name: bool,
                plain_name: usize,
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let config = extract_config(Path::new("synthetic.rs"), &file)
            .expect("demo file declares CONFIG_KEY and Config");
        let names: Vec<&str> = config.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["renamed-key", "plain_name"]);
    }

    #[test]
    fn config_field_honours_struct_rename_all() {
        // `rename_all = "kebab-case"` on the Config struct should
        // surface every Rust field's snake-case identifier with
        // dashes. A per-field `#[serde(rename)]` still wins.
        let source = r#"
            const CONFIG_KEY: &str = "perfectionist::demo";

            #[derive(serde::Deserialize)]
            #[serde(default, rename_all = "kebab-case")]
            struct Config {
                also_flag: bool,
                some_other_key: usize,
                #[serde(rename = "explicit")]
                overridden: bool,
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let config = extract_config(Path::new("synthetic.rs"), &file)
            .expect("demo file declares CONFIG_KEY and Config");
        let names: Vec<&str> = config.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["also-flag", "some-other-key", "explicit"]);
    }

    #[test]
    fn apply_rename_all_covers_serde_styles() {
        assert_eq!(
            apply_rename_all("snake_case", "BlockComment"),
            "block_comment"
        );
        assert_eq!(
            apply_rename_all("SCREAMING_SNAKE_CASE", "BlockComment"),
            "BLOCK_COMMENT",
        );
        assert_eq!(
            apply_rename_all("kebab-case", "BlockComment"),
            "block-comment"
        );
        assert_eq!(
            apply_rename_all("SCREAMING-KEBAB-CASE", "BlockComment"),
            "BLOCK-COMMENT",
        );
        assert_eq!(
            apply_rename_all("PascalCase", "BlockComment"),
            "BlockComment"
        );
        assert_eq!(
            apply_rename_all("camelCase", "BlockComment"),
            "blockComment"
        );
        assert_eq!(
            apply_rename_all("lowercase", "BlockComment"),
            "blockcomment"
        );
        assert_eq!(
            apply_rename_all("UPPERCASE", "BlockComment"),
            "BLOCKCOMMENT"
        );
        // Unknown style: pass through unchanged.
        assert_eq!(apply_rename_all("???", "BlockComment"), "BlockComment");
    }

    #[test]
    fn collect_referenced_idents_inspects_only_last_segment() {
        // `std` and `vec` would have been picked up by the old
        // every-segment walk; the current behaviour drops them.
        let ty = parse_type("std::vec::Vec<my_crate::Inner>");
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        collect_referenced_idents(&ty, &mut out, &mut seen);
        assert_eq!(out, vec!["Inner".to_owned()]);
    }
}
