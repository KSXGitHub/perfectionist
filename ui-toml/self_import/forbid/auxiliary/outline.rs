// Loaded as an outline module from `fixture.rs` via `#[path]`. The rule
// re-parses each of the crate's module source files (an out-of-line
// `mod outline;` body is not present in the crate-root AST), so this
// separate file's `self`-import is reached and flagged. This fixture
// guards against regressing to a crate-root-only walk that would skip
// separate-file submodules.
use crate::defs::inner::{self, Baz};
