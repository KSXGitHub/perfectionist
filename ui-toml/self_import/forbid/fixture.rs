#![allow(unused_imports, unused, reason = "ui fixture deliberately leaves imports unused")]

mod defs {
    pub mod inner {
        pub struct Baz;
        pub struct Qux;
    }
    pub mod sibling {
        pub struct Thing;
    }
}

// `use module::{self};` collapses to the bare module import.
mod brace_self {
    use crate::defs::inner::{self};
}

// `use module::{self, X};` splits into two statements at the item root.
mod brace_self_item {
    use crate::defs::inner::{self, Baz};
}

// The `self` leaf's rename is preserved on the bare module import.
mod brace_self_renamed {
    use crate::defs::inner::{self as renamed};
}

// The `module::self` form (only valid inside a brace list) drops `self`.
mod trailing_self_in_braces {
    use crate::defs::{inner::self};
}

// A nested sole-`self` group folds to the bare module path in place.
mod nested_self_sole {
    use crate::defs::{inner::{self}, sibling};
}

// A nested `{self, X}` group expands into sibling brace entries.
mod nested_self_item {
    use crate::defs::{inner::{self, Baz}, sibling};
}

// A bare module import is already compliant — no diagnostic.
mod clean {
    use crate::defs::sibling;
}

fn main() {}
