// aux-build:proc_macro_synth_binding.rs

// Regression test: `single_letter_function_param` must not fire on
// `fn _synth(x: u32) {}` parameters synthesised by a proc-macro derive
// whose expansion attaches a user-source span to the parameter
// identifier. The `SynthFnParam` derive imported below mirrors the
// `clap_derive` span shape on a minimal `#[synth_param]` attribute.

#![allow(dead_code, unused_variables, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthFnParam;

#[derive(SynthFnParam)]
#[synth_param]
struct UsesSynthFnParam;

fn main() {}
