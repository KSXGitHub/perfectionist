use std::io::Write;

// Bad, in a separate-file submodule: a `pub use` below a private import.
pub use std::io::Read;

mod deep;
