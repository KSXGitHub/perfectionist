// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `cfg_block_handling = "merge"`: a `#[cfg(...)]`-gated import is
// classified by its path, not hoisted into a trailing group. So a
// cfg-gated std import belongs *in* the std group.

// Bad: the cfg-gated std import is blank-separated from the other std
// import, but under `merge` both are in the std group and must sit
// together with no blank line between them.
use std::time::Duration;

#[cfg(unix)]
use std::collections::BTreeMap;

struct Sep1;

// Good: the cfg-gated std import sits directly with the std group.
use std::io::stdin;
#[cfg(unix)]
use std::collections::HashMap;

fn main() {}
