//! A crate whose `use` statements are laid out in the wrong order for
//! the multi-block style, which
//! `perfectionist::import_grouping_mismatch` reorders. The rule ships
//! inactive, and has no default layout to fall back on, so only a
//! run that both enables and configures it rewrites this.

pub mod local {
    /// An item of this crate's own.
    pub struct Marker(pub u8);
}

use crate::local::Marker;
use std::collections::BTreeSet;

/// What this crate contributes to the fixture's output.
pub fn report() -> String {
    let seen: BTreeSet<u8> = BTreeSet::from([Marker(9).0]);
    format!("import_grouping_mismatch: {}\n", seen.len())
}

#[cfg(test)]
mod tests {
    /// Both imports still resolve, which reordering the blocks they
    /// sit in cannot change.
    #[test]
    fn both_imports_still_resolve() {
        assert_eq!(super::report(), "import_grouping_mismatch: 1\n");
    }
}
