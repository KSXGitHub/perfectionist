//! Type-graph helpers used while extracting a rule's `Config`:
//! translating Rust types to TOML-flavoured labels for the field
//! table, and discovering which project-local types (enums and
//! structs) those fields reach so the renderer can show their shape.

use std::collections::BTreeSet;

use quote::ToTokens;
use syn::{Item, Type};

use crate::extract::serde_attrs::{apply_rename_all, doc_attrs_to_markdown, serde_str_attr};
use crate::extract::shared::SharedTypes;
use crate::model::{EnumVariant, StructField, TypeDoc, TypeKind};

/// Walk a `syn::Type` and collect every type identifier that isn't a
/// well-known built-in *or* a shared newtype carrying its own
/// definition-site TOML label. Only the *last* segment of each path
/// is considered, matching [`toml_type_label`]'s rule: leading
/// segments like `std::vec` in `std::vec::Vec<T>` are noise (the
/// generator only looks up types against the rule file's local
/// items, so qualified paths would never resolve anyway).
/// Insertion order matches the order each distinct ident is first
/// encountered, so the rendered docs list types in the order a
/// reader scanning the field list will meet them. The `seen` set
/// deduplicates across multiple fields. Shared types are skipped at
/// this layer (rather than dropped silently downstream by
/// [`find_type_doc`]) so the per-rule custom-types listing doesn't
/// accumulate idents the renderer already encoded into the
/// field-type column.
pub(crate) fn collect_referenced_idents(
    ty: &Type,
    out: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
    shared: &SharedTypes,
) {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                let name = segment.ident.to_string();
                if !is_builtin_type(&name) && !shared.contains(&name) && seen.insert(name.clone()) {
                    out.push(name);
                }
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(inner) = arg {
                            collect_referenced_idents(inner, out, seen, shared);
                        }
                    }
                }
            }
        }
        Type::Reference(type_ref) => collect_referenced_idents(&type_ref.elem, out, seen, shared),
        Type::Tuple(type_tuple) => {
            for elem in &type_tuple.elems {
                collect_referenced_idents(elem, out, seen, shared);
            }
        }
        Type::Array(type_array) => collect_referenced_idents(&type_array.elem, out, seen, shared),
        Type::Slice(type_slice) => collect_referenced_idents(&type_slice.elem, out, seen, shared),
        Type::Paren(type_paren) => collect_referenced_idents(&type_paren.elem, out, seen, shared),
        Type::Group(type_group) => collect_referenced_idents(&type_group.elem, out, seen, shared),
        _ => {}
    }
}

/// Whether `name` is in the renderer's built-in type set. Thin
/// wrapper around [`BUILTIN_TYPES`] kept for naming intent at the
/// call sites; the contract and rationale live on the constant.
pub(crate) fn is_builtin_type(name: &str) -> bool {
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
pub(crate) const BUILTIN_TYPES: &[&str] = &[
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
    "NonZeroU8",
    "NonZeroU16",
    "NonZeroU32",
    "NonZeroU64",
    "NonZeroU128",
    "NonZeroUsize",
    "NonZeroI8",
    "NonZeroI16",
    "NonZeroI32",
    "NonZeroI64",
    "NonZeroI128",
    "NonZeroIsize",
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
pub(crate) fn find_type_doc(
    file: &syn::File,
    ident: &str,
    shared: &SharedTypes,
) -> Option<TypeDoc> {
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
                            type_label: toml_type_label(&field.ty, shared),
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
/// - `NonZeroU*` / `NonZeroUsize` → `non-zero unsigned integer`
/// - `NonZeroI*` / `NonZeroIsize` → `non-zero integer`
/// - `f32` / `f64` → `float`
/// - `char` → `single-character string` (a TOML string of length
///   one, which is what `serde-toml` accepts for `char`). Stricter
///   subtypes that refine `char` — currently `AsciiLetter` from
///   `src/ascii_letter.rs`, whose own label is
///   `single-letter string` — are looked up via [`SharedTypes`]
///   rather than hard-coded here, so adding a new shared newtype
///   does not require editing this function.
/// - `String` / `&str` / `OsString` / `PathBuf` / `Cow<…>` → `string`
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
pub(crate) fn toml_type_label(ty: &Type, shared: &SharedTypes) -> String {
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
                "char" => "single-character string".to_owned(),
                "String" | "str" | "OsString" | "OsStr" | "Path" | "PathBuf" | "Cow" => {
                    "string".to_owned()
                }
                "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => "unsigned integer".to_owned(),
                "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => "integer".to_owned(),
                "NonZeroU8" | "NonZeroU16" | "NonZeroU32" | "NonZeroU64" | "NonZeroU128"
                | "NonZeroUsize" => "non-zero unsigned integer".to_owned(),
                "NonZeroI8" | "NonZeroI16" | "NonZeroI32" | "NonZeroI64" | "NonZeroI128"
                | "NonZeroIsize" => "non-zero integer".to_owned(),
                "f32" | "f64" => "float".to_owned(),
                "Vec" | "HashSet" | "BTreeSet" | "VecDeque" | "LinkedList" => {
                    match inner_types.first() {
                        Some(inner) => format!("[{}]", toml_type_label(inner, shared)),
                        None => "array".to_owned(),
                    }
                }
                "HashMap" | "BTreeMap" => match inner_types.get(1) {
                    Some(value) => format!("table of {}", toml_type_label(value, shared)),
                    None => "table".to_owned(),
                },
                "Option" | "Box" | "Rc" | "Arc" => match inner_types.first() {
                    Some(inner) => toml_type_label(inner, shared),
                    None => ident,
                },
                _ => shared.label_for(&ident).map(str::to_owned).unwrap_or(ident),
            }
        }
        Type::Reference(type_ref) => toml_type_label(&type_ref.elem, shared),
        Type::Paren(type_paren) => toml_type_label(&type_paren.elem, shared),
        Type::Group(type_group) => toml_type_label(&type_group.elem, shared),
        // Tuples, trait objects, function pointers, raw pointers,
        // etc. fall through to a Rust-syntax fallback. No current
        // `Config` field uses any of these; if one ever does, add
        // a real arm rather than letting Rust syntax leak into the
        // user-facing label.
        _ => ty.to_token_stream().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_type(source: &str) -> Type {
        syn::parse_str(source).expect("test input should parse as a syn::Type")
    }

    fn no_shared() -> SharedTypes {
        SharedTypes::default()
    }

    #[test]
    fn toml_type_label_primitives() {
        let shared = no_shared();
        assert_eq!(toml_type_label(&parse_type("bool"), &shared), "boolean");
        assert_eq!(
            toml_type_label(&parse_type("usize"), &shared),
            "unsigned integer"
        );
        assert_eq!(toml_type_label(&parse_type("i32"), &shared), "integer");
        assert_eq!(
            toml_type_label(&parse_type("NonZeroUsize"), &shared),
            "non-zero unsigned integer",
        );
        assert_eq!(
            toml_type_label(&parse_type("NonZeroU32"), &shared),
            "non-zero unsigned integer",
        );
        assert_eq!(
            toml_type_label(&parse_type("NonZeroIsize"), &shared),
            "non-zero integer",
        );
        assert_eq!(
            toml_type_label(&parse_type("NonZeroI64"), &shared),
            "non-zero integer",
        );
        assert_eq!(toml_type_label(&parse_type("f64"), &shared), "float");
        assert_eq!(toml_type_label(&parse_type("String"), &shared), "string");
        assert_eq!(
            toml_type_label(&parse_type("char"), &shared),
            "single-character string"
        );
    }

    #[test]
    fn toml_type_label_arrays_and_maps() {
        let shared = no_shared();
        assert_eq!(
            toml_type_label(&parse_type("Vec<char>"), &shared),
            "[single-character string]"
        );
        assert_eq!(
            toml_type_label(&parse_type("Vec<Vec<u8>>"), &shared),
            "[[unsigned integer]]"
        );
        assert_eq!(
            toml_type_label(&parse_type("HashMap<String, usize>"), &shared),
            "table of unsigned integer",
        );
    }

    #[test]
    fn toml_type_label_transparent_wrappers() {
        // Option, Box, Rc, Arc unwrap to the inner type.
        let shared = no_shared();
        assert_eq!(
            toml_type_label(&parse_type("Option<String>"), &shared),
            "string"
        );
        assert_eq!(
            toml_type_label(&parse_type("Box<usize>"), &shared),
            "unsigned integer"
        );
        assert_eq!(
            toml_type_label(&parse_type("Arc<Vec<String>>"), &shared),
            "[string]"
        );
    }

    #[test]
    fn toml_type_label_qualified_paths_use_last_segment() {
        let shared = no_shared();
        assert_eq!(
            toml_type_label(&parse_type("std::vec::Vec<String>"), &shared),
            "[string]"
        );
        assert_eq!(
            toml_type_label(&parse_type("alloc::string::String"), &shared),
            "string"
        );
    }

    #[test]
    fn toml_type_label_custom_idents_pass_through() {
        // Project-local enums/structs surface verbatim; readers
        // follow them to the per-rule Types subsection.
        let shared = no_shared();
        assert_eq!(toml_type_label(&parse_type("Scope"), &shared), "Scope");
        assert_eq!(
            toml_type_label(&parse_type("Vec<Scope>"), &shared),
            "[Scope]"
        );
    }

    #[test]
    fn toml_type_label_shared_newtype_uses_definition_label() {
        let tmp = std::env::temp_dir().join(format!(
            "perfectionist-gen-docs-ty-shared-{}",
            std::process::id(),
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("ascii_letter.rs"),
            r#"
                pub(crate) const TOML_LABEL: &str = "single-letter string";

                pub(crate) struct AsciiLetter(char);
            "#,
        )
        .unwrap();
        let shared = SharedTypes::discover(&tmp);
        assert_eq!(
            toml_type_label(&parse_type("AsciiLetter"), &shared),
            "single-letter string",
        );
        assert_eq!(
            toml_type_label(&parse_type("Vec<AsciiLetter>"), &shared),
            "[single-letter string]",
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn collect_referenced_idents_skips_builtins_and_keeps_order() {
        let ty = parse_type("HashMap<String, Vec<Scope>>");
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        collect_referenced_idents(&ty, &mut out, &mut seen, &no_shared());
        assert_eq!(out, vec!["Scope".to_owned()]);
    }

    #[test]
    fn collect_referenced_idents_inspects_only_last_segment() {
        // `std` and `vec` would have been picked up by the old
        // every-segment walk; the current behaviour drops them.
        let ty = parse_type("std::vec::Vec<my_crate::Inner>");
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        collect_referenced_idents(&ty, &mut out, &mut seen, &no_shared());
        assert_eq!(out, vec!["Inner".to_owned()]);
    }

    #[test]
    fn collect_referenced_idents_skips_shared_newtypes() {
        let tmp = std::env::temp_dir().join(format!(
            "perfectionist-gen-docs-ty-shared-collect-{}",
            std::process::id(),
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("ascii_letter.rs"),
            r#"
                pub(crate) const TOML_LABEL: &str = "single-letter string";
                pub(crate) struct AsciiLetter(char);
            "#,
        )
        .unwrap();
        let shared = SharedTypes::discover(&tmp);
        let ty = parse_type("Vec<AsciiLetter>");
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        collect_referenced_idents(&ty, &mut out, &mut seen, &shared);
        assert!(
            out.is_empty(),
            "shared newtypes should be skipped, got: {out:?}",
        );
        let _ = std::fs::remove_dir_all(&tmp);
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
        let shared = no_shared();
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
            let label = toml_type_label(&parse_type(&source), &shared);
            assert_ne!(
                label, name,
                "builtin `{name}` is missing a `toml_type_label` arm \
                 (rendered `{source}` as `{label}`); either add one \
                 or remove the entry from BUILTIN_TYPES",
            );
        }
    }
}
