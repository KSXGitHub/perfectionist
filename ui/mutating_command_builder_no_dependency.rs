// edition:2024
//
// The dependency gate at its default. This fixture declares no
// dependency on `command_extra`, so the by-value method the diagnostic
// would name does not exist here and the rule stays silent — even though
// the call below is one it fires on in `ui/mutating_command_builder.rs`,
// where the crate is present.
//
// The default also accepts a declaration in the surrounding workspace's
// own table, which is read from the manifest `CARGO_MANIFEST_DIR` leads
// to — this repository's. It declares `command-extra` per-package, so
// there is nothing there for the fixture to inherit.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::process::Command;

fn every_setter_but_no_crate_to_suggest() {
    let mut command = Command::new("ls");
    command.arg("-l");
    command.env("LANG", "C");
    command.env_clear();
}

fn main() {}
