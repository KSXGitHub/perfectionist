//! A crate cherry-picking a named item out of a prelude module, which
//! `perfectionist::named_prelude_imports` rewrites to the item's
//! canonical home.
//!
//! A second item is already imported from the canonical home, so the
//! rewrite leaves two `use` statements naming one module, which
//! `perfectionist::import_granularity_mismatch` merges. Both land in
//! one round: `cargo fix` recompiles after applying a suggestion and
//! reapplies until the crate reports nothing more, so the merge sees
//! the statement the first rewrite produced.

pub mod thing {
    /// An item imported through the prelude.
    pub struct Widget(pub u8);

    /// An item imported from its canonical home already.
    pub struct Gadget(pub u8);
}

pub mod prelude {
    pub use crate::thing::{Gadget, Widget};
}

use crate::prelude::Widget;
use crate::thing::Gadget;

/// What this crate contributes to the fixture's output.
pub fn report() -> String {
    format!("named_prelude_imports: {} {}\n", Widget(5).0, Gadget(6).0)
}

#[cfg(test)]
mod tests {
    /// Each item is the same item whichever path imports it.
    #[test]
    fn the_items_are_the_same_either_way() {
        assert_eq!(super::report(), "named_prelude_imports: 5 6\n");
    }
}
