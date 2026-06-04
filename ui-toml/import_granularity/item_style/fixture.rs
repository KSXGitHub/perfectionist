#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `style = "item"`: one `use` per leaf path.

// Bad: a braced list of two items from one module.
mod braced_list {
    use std::path::{Path, PathBuf};
}

// Bad: a statement crossing two modules.
mod cross_module {
    use std::{collections::HashMap, io::Read};
}

// Good: already one path per statement.
mod already_item {
    use std::path::Path;
    use std::path::PathBuf;
}

// Good: a glob is governed by `wildcard_imports`, not by this rule.
mod glob_left_alone {
    use std::io::*;
}

// Bad: a `self` import mixed with a sibling splits, but the `self` part
// keeps its irreducible braces.
mod self_with_sibling {
    use std::io::{self, Read};
}

// Good: a lone `self` import cannot be unbraced, so item style accepts
// it as-is.
mod lone_self {
    use std::fmt::{self};
}

fn main() {}
