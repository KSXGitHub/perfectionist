#![allow(unused_imports, unused, reason = "ui fixture deliberately leaves imports unused")]

mod defs {
    pub mod inner {
        pub struct Baz;
        pub struct Qux;
    }
}

// Bare module import followed by an item from it folds into `{self, X}`.
mod bare_then_item {
    use crate::defs::inner;
    use crate::defs::inner::Baz;
}

// The `{self}` form folds the same way, with no namespace change.
mod selfbrace_then_item {
    use crate::defs::inner::{self};
    use crate::defs::inner::Baz;
}

// The pair folds regardless of source order (item first, module second).
mod item_then_module {
    use crate::defs::inner::Baz;
    use crate::defs::inner;
}

// A braced item list under the module folds into the same `self` group.
mod module_then_group {
    use crate::defs::inner;
    use crate::defs::inner::{Baz, Qux};
}

// An intervening item breaks adjacency — no fold.
mod non_adjacent {
    use crate::defs::inner;
    struct Separator;
    use crate::defs::inner::Baz;
}

fn main() {}
