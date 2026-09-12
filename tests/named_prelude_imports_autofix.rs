//! End-to-end proof that `named_prelude_imports`' autofix keeps the
//! crate compiling.
//!
//! ## What this actually tests
//!
//! Not the suggestion text — the `ui/` sweep already pins that, and a
//! `.stderr` file cannot show applicability at all, which is exactly
//! the property that decides whether `cargo dylint --fix` touches a
//! suggestion. So the fixture is run through the real fixer and judged
//! on what it did to the source.
//!
//! `cargo fix` applies a crate's `MachineApplicable` suggestions,
//! recompiles, and on any new error throws the whole file's fixes away
//! with `errors present after applying fixes`. That makes one
//! over-confident suggestion worse than a merely wrong one: it also
//! costs every correct fix in the same crate. Asserting the fixer
//! stayed quiet is therefore the assertion that matters, and it is one
//! no `.stderr` can make.
//!
//! The two shapes that earned this test both come from the same root
//! cause — an item's *definition* path is not always a path the
//! importer may write:
//!
//! - `std`'s prelude re-exports items defined in `alloc`, which an
//!   ordinary crate has not linked.
//! - A `#[macro_export]` macro answers only to its crate root, not to
//!   the module its definition path names.
//!
//! Both once produced a machine-applicable rewrite that did not
//! compile. They are checked here alongside the shapes that *should*
//! be rewritten, so a future change cannot buy safety by giving up on
//! the fix.

pub mod _utils;

use _utils::{
    TempDir, build_project_with_config, cargo_manifest_dir, run_dylint_fix, shared_target_dir,
};
use std::fs;
use text_block_macros::text_block_fnl;

/// The sibling import rules would rewrite the same statements on their
/// own account, which would make "did the fixer touch this line?"
/// answer the wrong question.
const CONFIG: &str = text_block_fnl! {
    "[perfectionist]"
    r#"disable = ["import_granularity_mismatch", "import_grouping_mismatch", "wildcard_imports"]"#
};

const SOURCE: &str = text_block_fnl! {
    r##"#![allow(dead_code, unused_imports, reason = "fixture")]"##
    ""
    "pub mod thing {"
    "    #[macro_export]"
    "    macro_rules! shout {"
    "        () => {};"
    "    }"
    "    pub struct A;"
    "    pub fn helper() {}"
    "}"
    ""
    "pub mod prelude {"
    "    pub use crate::shout;"
    "    pub use crate::thing::{A, helper};"
    "}"
    ""
    "// Fixable: every entry is a local item the importer can name."
    "mod fixable_single {"
    "    use crate::prelude::A;"
    "}"
    "mod fixable_braced {"
    "    use crate::prelude::{A, helper};"
    "}"
    ""
    "// Not fixable: `Vec` and `String` are defined in `alloc`, which is"
    "// not linked here, so the definition path does not resolve."
    "mod foreign_crate_single {"
    "    use std::prelude::v1::Vec;"
    "}"
    "mod foreign_crate_braced {"
    "    use std::prelude::v1::{String, Vec};"
    "}"
    ""
    "// Not fixable: `shout` answers to `crate::shout`, not to the"
    "// `crate::thing` its definition path names."
    "mod exported_macro_single {"
    "    use crate::prelude::shout;"
    "}"
    "mod exported_macro_braced {"
    "    use crate::prelude::{A, shout};"
    "}"
};

/// Run the fixer over the fixture and hand back what it left on disk,
/// plus its stderr.
fn fix() -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    build_project_with_config(
        temp.path(),
        "named_prelude_imports_autofix",
        cargo_manifest_dir(),
        &[("src/lib.rs", SOURCE)],
        CONFIG,
    );
    let (stderr, success) = run_dylint_fix(temp.path(), &shared_target_dir());
    assert!(
        success,
        "`cargo dylint --fix` failed; stderr was:\n{stderr}",
    );
    let fixed = fs::read_to_string(temp.path().join("src/lib.rs")).expect("read fixed fixture");
    (temp, fixed, stderr)
}

#[test]
fn the_autofix_leaves_the_crate_compiling() {
    let (_temp, fixed, stderr) = fix();

    // The headline assertion. `cargo fix` prints this after applying a
    // suggestion that does not compile, having reverted the file — so
    // its absence is what says every applied rewrite was sound.
    assert!(
        !stderr.contains("errors present after applying fixes"),
        "the autofix produced code that does not compile; stderr was:\n{stderr}",
    );

    // A revert would also leave every line untouched, which would pass
    // the "not fixable" assertions below for the wrong reason. Pin the
    // rewrites that must have happened.
    for expected in ["use crate::thing::A;", "use crate::thing::{A, helper};"] {
        assert!(
            fixed.contains(expected),
            "expected the fixer to produce `{expected}`; it left:\n{fixed}",
        );
    }

    // The hazardous shapes keep their diagnostic but must not be
    // rewritten: their canonical path is not one this crate can name.
    for untouched in [
        "use std::prelude::v1::Vec;",
        "use std::prelude::v1::{String, Vec};",
        "use crate::prelude::shout;",
        "use crate::prelude::{A, shout};",
    ] {
        assert!(
            fixed.contains(untouched),
            "expected `{untouched}` to be left alone; the fixer left:\n{fixed}",
        );
    }
}
