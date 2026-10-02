// aux-build:proc_macro_synth_binding.rs
// aux-build:command_extra.rs
//
// Regression test: `folded_command_setter` must not fire on a fold
// synthesised by a proc-macro derive whose expansion attaches a
// user-source span to every token it emits. The
// `SynthFoldedCommandSetter` derive imported below mirrors the
// `clap_derive` span shape on a minimal
// `#[synth_folded_command_setter]` attribute.
//
// `command_extra` is pulled in because the synthesised fold names one of
// its setters, and `VARS` below is what the fold reads, so the shape is
// one the rule fires on when hand-written and the guard is the only
// thing left that can keep the fixture silent.

#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;
extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::{SynthFoldOwner, SynthFoldedCommandSetter};

const VARS: &[&str] = &["A", "B"];

#[derive(SynthFoldedCommandSetter)]
#[synth_folded_command_setter]
struct UsesSynthFoldedCommandSetter;

// The same fold written by hand inside the attribute, which the derive
// copies, spans and all, into a function it generates.
#[derive(SynthFoldOwner)]
#[synth_fold_owner(VARS.iter().fold(
    std::process::Command::new("ls"),
    command_extra::CommandExtra::without_env,
))]
struct UsesSynthFoldOwner;

fn main() {}
