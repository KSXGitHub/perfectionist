#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Default style is `grouped`: imports partitioned into std, internal,
// then third-party groups, each separated by exactly one blank line.
// A `#[cfg(...)]`-gated import forms an always-last (trailing) group.
//
// Fake third-party crates (`clap`, `serde`) and an internal module
// (`config`) so every import resolves; classification keys on the
// path's first segment, so a local `mod clap` stands in for the real
// crate. Each run of `use` statements is separated by a marker item so
// the runs stay isolated, and every import binds a distinct name so the
// whole crate-root file still compiles.

mod clap {
    pub struct Parser;
    pub struct Subcommand;
}
mod serde {
    pub struct Deserialize;
}
mod config {
    pub struct Args;
    pub struct Bytes;
    pub struct Opts;
    pub struct Cfg;
}

// Bad: third-party and internal imports intermixed with std, with no
// blank lines partitioning the groups.
use clap::Parser;
use std::time::Duration;
use crate::config::Args;

struct Sep1;

// Bad: groups are blank-separated but in the wrong order — third-party
// before std.
use serde::Deserialize;

use std::io::stdin;

struct Sep2;

// Bad: a blank line splits one std group in two.
use std::io::Read;

use std::time::Instant;

struct Sep3;

// Bad: std and internal groups not separated by a blank line.
use std::fmt::Write;
use crate::config::Bytes;

struct Sep4;

// Bad: a `#[cfg]`-gated import must form the always-last group, but it
// sits before the unconditional std import here.
#[cfg(unix)]
use std::collections::BTreeMap;
use std::mem::swap;

struct Sep5;

// Good: std, internal, third-party in order, each separated by one
// blank line.
use std::cmp::Ordering;

use crate::config::Opts;

use clap::Subcommand;

struct Sep6;

// Good: one std group with no internal blank lines.
use std::sync::Arc;
use std::path::Path;

struct Sep7;

// Good: the `#[cfg]`-gated import forms the trailing group, blank-
// separated from the std group above.
use std::rc::Rc;

#[cfg(unix)]
use std::collections::HashMap;

struct Sep8;

// Good: a lone import can't violate.
use std::ffi::OsStr;

struct Sep9;

// Good: a comment between two same-group imports is not a blank line, so
// it does not split the group.
use std::sync::Mutex;
// a note kept next to the import below
use std::sync::atomic::AtomicBool;

struct Sep10;

// Suppressed: an `#[allow]` on the run's enclosing module silences the
// rule even though std and internal are not blank-separated.
#[allow(perfectionist::import_grouping, reason = "ui fixture")]
mod suppressed {
    use std::hash::Hash;
    use crate::config::Cfg;
}

fn main() {}
