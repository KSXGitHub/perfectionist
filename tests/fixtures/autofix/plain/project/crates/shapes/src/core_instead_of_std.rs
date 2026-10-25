//! A module reaching an item through `core` that `std` re-exports
//! under the same suffix, which `perfectionist::core_instead_of_std`
//! rewrites to the `std` path. The rule ships inactive, so only a
//! run that enables it rewrites this.

use core::fmt::Display;

/// What this module contributes to the fixture's output.
pub fn report() -> String {
    let shown: &dyn Display = &7_u8;
    format!("core_instead_of_std: {shown}\n")
}

#[cfg(test)]
mod tests {
    /// The trait is the same trait whichever crate's path names it.
    #[test]
    fn the_trait_is_the_same_either_way() {
        assert_eq!(super::report(), "core_instead_of_std: 7\n");
    }
}
