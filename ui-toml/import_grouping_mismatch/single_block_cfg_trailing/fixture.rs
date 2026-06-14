#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `style = "single_block"` with `cfg_trailing_block = true`: the
// non-cfg imports stay in one contiguous block and every
// `#[cfg(...)]`-gated import is hoisted into a single trailing block one
// blank line below. Path origin is irrelevant — only the cfg gate moves
// an import.

// Bad: a cfg-gated import sits inside the main block. The fix keeps the
// non-cfg imports in one block and moves the cfg-gated import into a
// trailing block.
use std::time::Duration;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::cmp::Ordering;

struct Sep1;

// Bad: a blank line splits two non-cfg imports; single_block admits no
// blank line within the main block.
use std::io::stdin;

use std::process::exit;

struct Sep2;

// Good: the non-cfg imports sit in one block and the cfg-gated import
// trails one blank line below.
use std::collections::HashMap;
use std::env::args;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn main() {}
