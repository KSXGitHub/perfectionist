// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// aux-build:proc_macro_synth_binding.rs

// Regression test: `single_letter_closure_param` must not fire on
// `|x| { ... }` closure parameters synthesised by a proc-macro derive
// whose expansion attaches a user-source span to the parameter
// identifier. The `SynthClosure` derive imported below mirrors the
// `clap_derive` span shape on a minimal `#[synth_closure]` attribute.

#![allow(dead_code, unused_variables, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthClosure;

#[derive(SynthClosure)]
#[synth_closure]
struct UsesSynthClosure;

fn main() {}
