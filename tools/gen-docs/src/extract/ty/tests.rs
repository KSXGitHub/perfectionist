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
        "unsigned integer",
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
        "single-character string",
    );
}

#[test]
fn toml_type_label_arrays_and_maps() {
    let shared = no_shared();
    assert_eq!(
        toml_type_label(&parse_type("Vec<char>"), &shared),
        "[single-character string]",
    );
    assert_eq!(
        toml_type_label(&parse_type("Vec<Vec<u8>>"), &shared),
        "[[unsigned integer]]",
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
        "string",
    );
    assert_eq!(
        toml_type_label(&parse_type("Box<usize>"), &shared),
        "unsigned integer",
    );
    assert_eq!(
        toml_type_label(&parse_type("Arc<Vec<String>>"), &shared),
        "[string]",
    );
}

#[test]
fn toml_type_label_qualified_paths_use_last_segment() {
    let shared = no_shared();
    assert_eq!(
        toml_type_label(&parse_type("std::vec::Vec<String>"), &shared),
        "[string]",
    );
    assert_eq!(
        toml_type_label(&parse_type("alloc::string::String"), &shared),
        "string",
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
        "[Scope]",
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
