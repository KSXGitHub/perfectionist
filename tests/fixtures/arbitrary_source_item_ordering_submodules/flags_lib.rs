#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, unused_imports, dead_code, reason = "fixture")]

pub mod inline {
    // Bad, in an inline module: a `pub mod` below a `pub use`.
    pub use std::path::PathBuf;

    pub mod nested {}
}

mod separate;

use std::collections::HashMap;

// Bad, in the crate root: a `pub use` below a private import.
pub use std::path::Path;
