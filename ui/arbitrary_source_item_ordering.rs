// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// The enforced order is `pub mod`, then `pub use`, then private imports
// and every other item. Each case lives in its own module so the
// high-water mark of one does not leak into the next.

// Good: an `extern crate` is transparent to the ordering, so it does not
// close the leading section the way a plain item would. That is what
// keeps `#[macro_use] extern crate foo;` legal above the `pub mod`
// declarations of a crate root.
extern crate core;

pub mod extern_crate_first {
    pub struct Marker;
}

// Bad: a `pub mod` below the `pub use` that re-exports from it.
mod pub_mod_after_pub_use {
    pub mod printer {
        pub struct Printer;
    }

    pub use self::printer::Printer;

    pub mod parser {}
}

// Bad: a `pub use` below a private import.
mod pub_use_after_private_use {
    use std::collections::HashMap;

    pub use std::path::Path;
}

// Bad: a `pub mod` below a private import.
mod pub_mod_after_private_use {
    use std::collections::BTreeMap;

    pub mod nested {}
}

// Bad: a `pub use` below a plain item.
mod pub_use_after_item {
    fn helper() {}

    pub use std::path::Component;
}

// Bad: a `pub mod` below a plain item. The blocker is the first item to
// reach the trailing section, so both offenders point at the `struct`
// rather than at the private `mod` between them.
mod pub_mod_after_item {
    struct Registry;

    mod internals {}

    pub mod first {}

    pub mod second {}
}

// Good: the prescribed order, top to bottom.
mod ordered {
    pub mod parser {
        pub struct Parser;
    }

    pub use self::parser::Parser;

    use std::collections::VecDeque;

    fn helper() {}
}

// Good: private imports and other items share the trailing section, so
// neither has to precede the other.
mod trailing_section_unordered {
    fn first() {}

    use std::collections::BinaryHeap;

    struct Second;

    use std::collections::LinkedList;
}

// Good: a `#[cfg(...)]`-gated import block is excused from the ordering,
// so it may trail the main import block whatever its visibility.
mod trailing_cfg_imports {
    use std::collections::BTreeSet;

    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;

    #[cfg(unix)]
    pub use std::os::unix::ffi::OsStringExt;
}

// Good: being excused, a gated import does not close the leading section
// either — the `pub mod` below it is still in the right place.
mod cfg_import_does_not_pin {
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;

    pub mod nested {}
}

// Good: only the top-level sequence of a module body is ordered. The
// module inside this function body would be flagged if the rule
// descended into function bodies.
mod fn_body_module_ignored {
    fn build() {
        mod inner {
            pub use std::path::PathBuf;

            pub mod nested {}
        }
    }
}

// Good: an `#[expect]` on the offending item silences it, which
// anchoring the finding at the enclosing HIR node is what makes
// possible. `#[expect]` rather than `#[allow]` so the case fails both
// ways: an unanchored finding leaks a warning here, and one anchored
// somewhere else leaves the expectation unfulfilled.
mod expected_offender {
    pub use std::path::Prefix;

    #[expect(
        perfectionist::arbitrary_source_item_ordering,
        reason = "the anchor must resolve a suppression written on the item"
    )]
    pub mod nested {}
}

fn main() {}
