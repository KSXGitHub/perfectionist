#![allow(unused_imports, unused, reason = "ui fixture deliberately leaves imports unused")]

mod defs {
    pub mod inner {
        pub struct Baz;
        pub struct Qux;
    }
    pub mod sibling {
        pub struct Thing;
    }
    pub mod r#match {
        pub struct Token;
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

// A raw-identifier module renders with its `r#` prefix intact.
mod raw_ident {
    use crate::defs::r#match::{self};
}

// The split preserves the item's attributes on both statements.
mod attr_split {
    #[allow(unused_imports, reason = "self_import attribute-preservation fixture")]
    use crate::defs::inner::{self, Baz};
}

// The split preserves visibility on both synthesised statements.
mod pub_split {
    pub use crate::defs::inner::{self, Baz};
}

// A `{self, *}` group splits the module import out from the glob.
mod glob_with_self {
    use crate::defs::inner::{self, *};
}

// A bare module import is already compliant — no diagnostic.
mod clean {
    use crate::defs::sibling;
}

fn main() {}
