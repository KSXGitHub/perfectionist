#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    ambiguous_glob_reexports,
    hidden_glob_reexports,
    reason = "ui fixture"
)]

// A local module to glob-import from, plus a `prelude` module so the
// prelude exception has something real to resolve against. The rule is
// purely syntactic, but keeping the paths resolvable avoids unrelated
// "unresolved import" errors in the fixture.
mod thing {
    pub struct A;
    pub struct B;
}

mod prelude {
    pub use crate::thing::{A, B};
}

// Not flagged: the prelude exception — the final non-glob segment is
// `prelude`.
use crate::prelude::*;

// Not flagged: the root-re-export exception — a bare-`pub` glob at the
// top level of a module body.
pub use crate::thing::*;

// Bad: a `pub(crate)` glob is a narrower re-export, not the bare-`pub`
// root-re-export exception, so it is still flagged.
pub(crate) use crate::thing::*;

// Bad: a plain glob, neither a prelude nor a `pub` re-export.
use crate::thing::*;

mod nested {
    // Bad: the canonical `use super::*;` the rule targets, here in an
    // always-live inline module (the `#[cfg(test)] mod tests` shape is
    // covered by the on-disk submodule integration test).
    use super::*;
}

mod outer {
    mod inner {
        // Bad: a glob two inline-module levels deep is still reached —
        // `live_module_spans` is built from the crate-wide
        // `hir_free_items()`, which recurses through nested modules, so
        // the descent guard does not stop at the first level.
        use crate::thing::*;
    }
}

// Bad: a glob nested inside a brace list is flagged; the sibling named
// leaf is left alone.
use crate::thing::{A, *};

fn main() {}
