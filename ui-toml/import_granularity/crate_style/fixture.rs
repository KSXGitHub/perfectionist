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

mod thing {
    pub struct T;
}

// Bad: a module imported on its own line plus an item from it folds
// into the `self` form.
mod self_merge {
    use crate::thing;
    use crate::thing::T;
}

fn main() {}
