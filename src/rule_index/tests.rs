//! Unit tests for the index's invariants: its entries are in the
//! order [`super::is_registered_lint`] searches them in, every entry
//! names a rule module that declares the matching lint, and every
//! rule module is named by an entry.
//!
//! Reading the names out of the `LintStore` made the last two hold
//! by construction. A written-out index can drift instead: an entry
//! whose spelling parts ways with the name its `declare_tool_lint!`
//! block declares leaves `unknown_perfectionist_lints` reporting a
//! shipped lint as unknown, and a rule module the index never names
//! registers nothing at all.

use super::LINT_NAMES;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn rules_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules")
}

/// The lint `source`'s `declare_tool_lint!` block declares, in the
/// snake_case form the macro derives from the constant's identifier,
/// or `None` for a source file that declares no lint at all.
fn declared_lint_name(source: &str) -> Option<String> {
    let (_, after_prefix) = source.split_once("pub perfectionist::")?;
    let (identifier, _) = after_prefix.split_once(',')?;
    Some(identifier.trim().to_lowercase())
}

fn read_rule_source(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn entries_are_in_ascending_order() {
    let disorder = LINT_NAMES.windows(2).find(|pair| pair[0] >= pair[1]);
    assert_eq!(disorder, None, "`rule_index!` entries are out of order");
}

#[test]
fn every_entry_names_a_module_that_declares_it() {
    for name in LINT_NAMES {
        let path = rules_dir().join(format!("{name}.rs"));
        let declared = declared_lint_name(&read_rule_source(&path));
        assert_eq!(
            declared.as_deref(),
            Some(*name),
            "`{name}` is indexed, but {} declares {declared:?}",
            path.display(),
        );
    }
}

#[test]
fn every_declared_lint_is_indexed() {
    let indexed: BTreeSet<&str> = LINT_NAMES.iter().copied().collect();
    let entries = fs::read_dir(rules_dir()).expect("failed to read src/rules/");
    for entry in entries {
        let path = entry.expect("failed to read directory entry").path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let Some(declared) = declared_lint_name(&read_rule_source(&path)) else {
            continue;
        };
        assert!(
            indexed.contains(declared.as_str()),
            "{} declares `{declared}`, which no `rule_index!` entry names",
            path.display(),
        );
    }
}
