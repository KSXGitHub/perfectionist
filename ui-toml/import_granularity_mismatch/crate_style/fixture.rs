// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `style = "crate"`: one `use` per crate root, every shared
// prefix collapsed into nested braces.

// Bad: four statements all rooted at `std`.
mod many_statements {
    use std::collections::HashMap;
    use std::io::Read;
    use std::path::Path;
    use std::path::PathBuf;
}

// Bad: a single statement that isn't fully collapsed (the `path`
// prefix repeats).
mod not_collapsed {
    use std::{path::Path, path::PathBuf};
}

// Good: already one collapsed `use` for the crate root.
mod already_crate {
    use std::{collections::HashMap, io::Read, mem::swap};
}

// Good: two distinct bindings of the same path can't collapse further,
// so the single braced statement is already canonical.
mod rename_dup {
    use std::cmp::{Ordering, Ordering as Ordering2};
}

// Bad: a glob and a sibling item from the same module collapse into one
// braced statement (and the result, carrying a bare `*`, is stable).
mod glob_sibling {
    use std::io::Read;
    use std::io::*;
}

mod thing {
    pub struct T;
}

// Bad: a bare import plus an item from a module of the same name share a
// crate root and must merge into one `use`. The merge is ambiguous — the
// `self`-fold `crate::thing::{self, T}` and the sibling-split
// `crate::{thing, thing::T}` are not interchangeable (the fold re-imports
// only the module `thing`, dropping any value or macro named `thing`), so
// the rule offers both as `MaybeIncorrect` suggestions and lets the
// author pick.
mod self_merge {
    use crate::thing;
    use crate::thing::T;
}

// Good: an explicit `self` import is already a valid, compiling collapsed
// form (the two brace entries name different things — the module via
// `self` and the item `T`), so it is left alone, never rewritten into the
// sibling shape.
mod explicit_self {
    use crate::thing::{self, T};
}

// Good: the sibling-split form of the very same imports — the bare module
// `thing` next to the item `T`. It is already one `use` per crate root
// and preserves every namespace `thing` binds, so it is the minimal
// crate-granularity shape and is left alone — never "collapsed" further
// into the `self` form, which the rule cannot prove is equivalent.
mod sibling_form {
    use crate::{thing, thing::T};
}

// A name that is both a module and a value: `dual` is a module *and* a
// function re-exported at the crate root under the same name.
mod dual {
    pub fn dual() -> u32 {
        0
    }
    pub struct Opts;
}
pub use dual::dual;

// Good: a single statement importing the value `dual` *and* an item
// from the module `dual` is already the minimal crate-granularity form.
// Collapsing it to `crate::dual::{self, Opts}` would re-import only the
// module, dropping the value `dual` and breaking the call below — so the
// rule must leave it alone.
// See <https://github.com/KSXGitHub/perfectionist/issues/186>.
mod dual_ns {
    use crate::{dual, dual::Opts};

    fn use_them() -> u32 {
        let _opts = Opts;
        dual()
    }
}

// Good: a bare crate-root import (`use std;`) can't be folded with a
// deeper import from the same root (`use std::io::Write;`). The only
// one-statement form is `std::{self, io::Write}`, whose `self` re-imports
// the crate alone — so there's no safe one-`use`-per-root shape and the
// split is left alone rather than rewritten into a `self` merge.
mod bare_root_item {
    use std;
    use std::io::Write;
}

fn main() {}
