#![allow(unused_imports, unused, reason = "ui fixture deliberately leaves imports unused")]

mod defs {
    pub mod inner {
        pub struct Baz;
        pub struct Qux;
    }
    pub mod r#match {
        pub struct Token;
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

// A raw-identifier module renders with its `r#` prefix intact.
mod raw_ident {
    use crate::defs::r#match;
    use crate::defs::r#match::Token;
}

// Matching attributes on both statements fold into one (attrs kept).
mod attr_matching {
    #[allow(unused_imports, reason = "self_import matching-attrs fixture")]
    use crate::defs::inner;
    #[allow(unused_imports, reason = "self_import matching-attrs fixture")]
    use crate::defs::inner::Baz;
}

// Matching visibility folds and is preserved on the merged statement.
mod pub_matching {
    pub use crate::defs::inner;
    pub use crate::defs::inner::Baz;
}

// Mismatched attributes break the fold — no diagnostic.
mod attr_mismatch {
    use crate::defs::inner;
    #[allow(unused_imports, reason = "self_import mismatched-attrs fixture")]
    use crate::defs::inner::Baz;
}

// Mismatched visibility breaks the fold — no diagnostic.
mod vis_mismatch {
    pub use crate::defs::inner;
    use crate::defs::inner::Baz;
}

// Two absolute imports fold and the leading `::` is preserved.
mod root_match {
    use ::std::collections;
    use ::std::collections::HashMap;
}

// An absolute and a relative import must NOT fold (`::std` vs `std`).
mod root_mismatch {
    use ::std::collections;
    use std::collections::HashMap;
}

// An intervening item breaks adjacency — no fold.
mod non_adjacent {
    use crate::defs::inner;
    struct Separator;
    use crate::defs::inner::Baz;
}

fn main() {}
