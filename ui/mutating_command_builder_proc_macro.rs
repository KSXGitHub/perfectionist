// aux-build:proc_macro_synth_binding.rs
// aux-build:command_extra.rs

// Regression test: `mutating_command_builder` must not fire on a
// `Command` setter call synthesised by a proc-macro derive whose
// expansion attaches a user-source span to the method segment. The
// `SynthCommandSetter` derive imported below mirrors the `clap_derive`
// span shape on a minimal `#[synth_command_setter]` attribute.
//
// `command_extra` is pulled in so the rule's dependency gate passes and
// the guard is the only thing left that can keep the fixture silent.

#![allow(dead_code, unused_variables, reason = "ui fixture")]

extern crate command_extra;
extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthCommandSetter;

#[derive(SynthCommandSetter)]
#[synth_command_setter]
struct UsesSynthCommandSetter;

fn main() {}
