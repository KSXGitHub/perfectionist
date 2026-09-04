// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `style = "single_block"` with `cfg_block_handling = "merge"`:
// cfg-gated imports are not separated. Every import — cfg-gated or not —
// belongs in one contiguous block with no blank lines.

// Bad: a blank line carves the cfg-gated import into its own block;
// `merge` keeps it in the one block with no blank line.
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

struct Sep1;

// Good: the cfg-gated import sits directly in the one block.
use std::io::stdin;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

fn main() {}
