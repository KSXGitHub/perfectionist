#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `style = "crate"` with `self_merge = "split"`: a path segment that
// names both an item and a module is always written as the sibling-split
// `use crate::{thing, thing::T};`.

mod thing {
    pub struct T;
}

// Bad: the `self`-fold single statement is now flagged and rewritten to
// the split. Raising the module-only `self` to a bare item binds the name
// in every namespace, so it stays `MaybeIncorrect` (shown as a diff).
mod fold_to_split {
    use crate::thing::{self, T};
}

// Bad: two statements that must merge collapse to the split. No `self` is
// raised here, so the fix is machine-applicable.
mod merge_to_split {
    use crate::thing;
    use crate::thing::T;
}

// Good: already the split shape, left untouched.
mod already_split {
    use crate::{thing, thing::T};
}

// Good: a lone module-only `self` has no deeper sibling to split against,
// so it is left alone.
mod lone_self {
    use crate::thing::{self};
}

fn main() {}
