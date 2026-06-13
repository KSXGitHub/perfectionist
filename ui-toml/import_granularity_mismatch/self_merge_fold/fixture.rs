#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `style = "crate"` with `self_merge = "fold"`: a path segment that
// names both an item and a module is always written as the `self`-fold
// `use crate::thing::{self, T};`.

mod thing {
    pub struct T;
}

// Bad: the sibling-split single statement is now flagged and rewritten to
// the fold. The rewrite drops any value or macro bound by the bare name,
// so it stays `MaybeIncorrect` (shown as a diff).
mod sibling_to_fold {
    use crate::{thing, thing::T};
}

// Bad: two statements that must merge collapse straight to the fold.
mod merge_to_fold {
    use crate::thing;
    use crate::thing::T;
}

// Good: already the fold shape, left untouched.
mod already_fold {
    use crate::thing::{self, T};
}

// Good: a plain crate merge with no dual name collapses normally, with a
// machine-applicable fix.
mod plain_merge {
    use std::collections::HashMap;
    use std::io::Read;
}

fn main() {}
