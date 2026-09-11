// A library crate whose prelude re-exports an item defined elsewhere in
// it, so a cherry-pick from that prelude has an external canonical
// module to be re-pointed at.

pub mod thing {
    pub struct Item;
}

pub mod prelude {
    pub use crate::thing::Item;
}
