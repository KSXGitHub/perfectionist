// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, dead_code, unused_imports, reason = "ui fixture")]

extern crate alloc;

// Each case sits in its own module so that the sibling
// `import_granularity_mismatch` rule has nothing to merge.

// Bad: an item `std` reaches by the same suffix, written through `core`.
mod plain_core_path {
    use core::fmt::Display;
}

// Bad: the same through `alloc`, which the default config covers.
mod plain_alloc_path {
    use alloc::sync::Arc;
}

// Bad: a `::`-rooted path names the same crate, so it is flagged too.
mod rooted_path {
    use ::core::hash::Hash;
}

// Good: already written through `std`.
mod already_std {
    use std::fmt::Debug;
}

// Bad, once: the leaves of a brace list share one `core` token, and one
// rewrite of that token moves every one of them.
mod brace_list {
    use core::fmt::{Binary, Octal};
}

// Good: `std::panic::PanicInfo` is a deprecated alias of
// `PanicHookInfo`, not the type `core::panic::PanicInfo` names, so
// rewriting the crate segment would change what the path means.
mod renamed_under_std {
    use core::panic::PanicInfo;
}

// Bad, with a `help` instead of a rewrite: `Location` is reachable
// through `std`, but it shares its `core` token with `PanicInfo`, which
// is not.
mod brace_list_with_a_blocked_leaf {
    use core::panic::{Location, PanicInfo};
}

// Bad: a path in type position is a path like any other.
mod type_position {
    pub fn describe(_address: core::net::IpAddr) {}
}

// Bad: so is one in expression position.
mod expression_position {
    pub fn null_pointer() -> *const u32 {
        core::ptr::null::<u32>()
    }
}

// Good: `core::panic!` and `std::panic!` are different macros, so a
// name imported into the macro namespace is left alone.
mod macro_namespace {
    use core::assert_eq;
}

// Bad, once: `alloc::vec` names a module and a macro at once, and only
// the module half is the rule's business.
mod module_and_macro {
    use alloc::vec;
}

// Good: a `core` that is not the crate is not this rule's business.
mod shadowed_crate_name {
    pub mod core {
        pub mod fmt {
            pub struct Display;
        }
    }

    pub fn shadowed() -> core::fmt::Display {
        core::fmt::Display
    }
}

fn main() {}
