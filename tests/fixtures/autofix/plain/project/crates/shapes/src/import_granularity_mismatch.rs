//! A module splitting one module across two `use` statements, which
//! `perfectionist::import_granularity_mismatch` merges.

use std::collections::BTreeMap;
use std::collections::HashMap;

/// What this module contributes to the fixture's output.
pub fn report() -> String {
    let ordered: BTreeMap<u8, u8> = BTreeMap::from([(1, 2)]);
    let hashed: HashMap<u8, u8> = HashMap::from([(3, 4)]);
    format!(
        "import_granularity_mismatch: {} {}\n",
        ordered.len(),
        hashed.len(),
    )
}

#[cfg(test)]
mod tests {
    /// Both maps are still reached, which merging the statements that
    /// import them cannot change.
    #[test]
    fn both_maps_are_still_reached() {
        assert_eq!(super::report(), "import_granularity_mismatch: 1 1\n");
    }
}
