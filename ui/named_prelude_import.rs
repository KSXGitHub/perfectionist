#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, dead_code, unused_imports, non_snake_case, reason = "ui fixture")]

// A prelude module re-exporting items from their canonical home, so the
// fix has a real `DefId` to resolve a canonical path against. Each case
// sits in its own module so the per-rule warnings stay isolated (and so
// the sibling `import_granularity` rule has nothing to merge).
pub mod thing {
    pub struct A;
    pub struct B;
    pub fn helper() {}
}

pub mod prelude {
    pub use crate::thing::{A, B, helper};
}

// Bad: a named item cherry-picked from the prelude. Fixable to its
// canonical module `crate::thing`.
mod named {
    use crate::prelude::A;
}

// Bad: same, with an `as` rename preserved by the fix.
mod renamed {
    use crate::prelude::B as Renamed;
}

// Bad: a brace list flags each leaf in turn.
mod braced {
    use crate::prelude::{A, helper};
}

// Not flagged: the glob form is the canonical prelude shape.
mod glob_ok {
    use crate::prelude::*;
}

// Not flagged: importing the prelude module itself is not a cherry-pick.
mod module_ok {
    use crate::prelude as p;
}

// A name re-exported into a prelude in two namespaces from two different
// modules: the type `Dual` (a braced struct, type namespace only) and the
// function `Dual` (value namespace). The import pulls in both, so no single
// `use` reproduces it — the leaf is still flagged, but with a `help`, not a
// machine-applicable rewrite that would silently drop one namespace.
pub mod type_home {
    pub struct Dual {
        pub field: u8,
    }
}
pub mod value_home {
    pub fn Dual() {}
}
pub mod multi {
    pub mod prelude {
        pub use crate::type_home::Dual;
        pub use crate::value_home::Dual;
    }
}
mod multi_ns {
    use crate::multi::prelude::Dual;
}

fn main() {}
