//! Unit tests that keep the index and `src/rules/` in step: every
//! entry names a rule module that declares the matching lint, and
//! every rule module is named by an entry.
//!
//! The `LintStore` used to answer the first question by construction
//! — the names came out of it — and the compiler answers the second
//! one, since a rule module implements [`super::RuleRegistration`]
//! for a marker type only the index defines. What neither catches is
//! an entry whose spelling has drifted from the name its
//! `declare_tool_lint!` block declares, which would leave
//! `unknown_perfectionist_lints` reporting a shipped lint as unknown.

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
