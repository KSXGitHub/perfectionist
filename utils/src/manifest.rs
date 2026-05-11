//! Build a fixture's `Cargo.toml` and `dylint.toml` as Rust values
//! and serialise them. The Cargo manifest is a `cargo_toml::Manifest`
//! with the perfectionist-specific workspace metadata grafted onto
//! its `Metadata` generic; the dylint config is the same `Manifest`
//! type carrying just a `workspace.metadata.dylint` payload.

use std::path::Path;

use cargo_toml::{Edition, Inheritable, Manifest, Package, Product, Workspace};
use serde::Serialize;

#[derive(Default, Serialize)]
pub struct DylintWorkspaceMetadata {
    pub dylint: DylintMetadata,
}

#[derive(Default, Serialize)]
pub struct DylintMetadata {
    pub libraries: Vec<DylintLibrary>,
}

#[derive(Default, Serialize)]
pub struct DylintLibrary {
    pub path: String,
}

pub fn fixture_cargo_toml(package_name: &str) -> String {
    let mut package = Package::<()>::new(package_name, "0.0.0");
    package.edition = Inheritable::Set(Edition::E2024);
    let manifest = Manifest::<()> {
        package: Some(package),
        lib: Some(Product {
            path: Some("src/lib.rs".to_owned()),
            ..Default::default()
        }),
        // Declare an empty workspace so cargo doesn't walk up the
        // filesystem and try to enroll the fixture into the
        // perfectionist workspace it happens to be nested inside.
        workspace: Some(Workspace::default()),
        ..Default::default()
    };
    toml::to_string(&manifest).expect("serialize Cargo.toml")
}

pub fn fixture_dylint_toml(perfectionist_dir: &Path) -> String {
    fixture_dylint_toml_with_config(perfectionist_dir, "")
}

/// Like [`fixture_dylint_toml`], but appends `extra_toml` after the
/// `workspace.metadata.dylint` block. The caller is responsible for
/// supplying valid TOML — typically a per-rule configuration table
/// keyed by the rule's namespaced name, e.g.:
///
/// ```toml
/// ["perfectionist::macro_trailing_comma"]
/// extra_name_based = ["my_macro"]
/// ```
pub fn fixture_dylint_toml_with_config(perfectionist_dir: &Path, extra_toml: &str) -> String {
    let library = DylintLibrary {
        path: perfectionist_dir.display().to_string(),
    };
    let manifest = Manifest::<DylintWorkspaceMetadata> {
        workspace: Some(Workspace {
            metadata: Some(DylintWorkspaceMetadata {
                dylint: DylintMetadata {
                    libraries: vec![library],
                },
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let base = toml::to_string(&manifest).expect("serialize dylint.toml");
    if extra_toml.is_empty() {
        base
    } else {
        format!("{base}\n{extra_toml}")
    }
}
