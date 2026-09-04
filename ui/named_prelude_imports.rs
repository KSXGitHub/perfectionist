#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, dead_code, unused_imports, non_snake_case, reason = "ui fixture")]

// A prelude module re-exporting items from their canonical home, so the
// fix has a real `DefId` to resolve a canonical path against. Each case
// sits in its own module so the per-rule warnings stay isolated (and so
// the sibling `import_granularity_mismatch` rule has nothing to merge).
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

// Bad: same, with an `as` rename. The replaced span stops before the
// rename, so the fix leaves it in place rather than reproducing it.
mod renamed {
    use crate::prelude::B as Renamed;
}

// Bad: a brace list flags each leaf in turn. The fix rebuilds the whole
// tree, so it is offered once — on the first leaf — and both entries
// land back under the prefix they now share.
mod braced {
    use crate::prelude::{A, helper};
}

// Bad: a brace list whose entries keep their renames when the tree is
// rebuilt around the canonical prefix.
mod braced_rename {
    use crate::prelude::{A as First, B as Second};
}

// Bad: a redundant one-entry brace list. The rebuilt tree drops the
// braces along with the prelude.
mod braced_single {
    use crate::prelude::{helper};
}

// Bad: only the named entry is a cherry-pick. The glob is left as
// written, and the rebuilt tree folds out only the prefix the two
// actually share.
mod braced_glob {
    use crate::prelude::{A, *};
}

// Bad, but not fixed: rebuilding the tree around `crate` would turn the
// `self` entry into a bare `use crate::prelude;`, which binds the name
// in every namespace instead of just the module.
mod braced_self {
    use crate::prelude::{self, A};
}

// Bad: the statement's attributes sit outside the replaced span, so the
// rewrite carries them without having to reproduce them.
mod attributed {
    /// A doc comment and a `#[cfg(...)]`, both outside the rewrite.
    #[cfg(not(test))]
    pub use crate::prelude::{A, helper};
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

// The same name inside a brace list: one entry cannot be re-pointed, so
// the whole tree stays unrewritten and every entry carries a `help`.
mod multi_ns_braced {
    use crate::multi::prelude::{Dual};
}

// A keyword-named module (`r#type`) re-exported through a prelude: the
// canonical-module fix must round-trip the keyword as a raw identifier
// (`crate::r#type::Thing`), not the bare `crate::type::Thing` that would
// fail to parse.
pub mod r#type {
    pub struct Thing;
}
pub mod kw_prelude {
    pub mod prelude {
        pub use crate::r#type::Thing;
    }
}
mod kw_cherry {
    use crate::kw_prelude::prelude::Thing;
}

// One prelude curating items from two modules: the rebuilt tree can only
// fold out the crate root the two canonical modules share.
pub mod other {
    pub struct C;
}
pub mod mixed {
    pub mod prelude {
        pub use crate::other::C;
        pub use crate::thing::A;
    }
}
mod mixed_cherry {
    use crate::mixed::prelude::{A, C};
}

fn main() {}
