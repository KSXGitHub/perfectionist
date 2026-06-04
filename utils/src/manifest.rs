//! Build a fixture's `Cargo.toml` and `dylint.toml` as Rust values
//! and serialise them. The Cargo manifest is a `cargo_toml::Manifest`
//! with the perfectionist-specific workspace metadata grafted onto
//! its `Metadata` generic; the dylint config is the same [`Manifest`]
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
    toml::to_string(&manifest).expect("serialize dylint.toml")
}
