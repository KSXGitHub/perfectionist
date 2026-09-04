// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// aux-build:proc_macro_synth_binding.rs

// Regression test: `single_letter_const_generic` must not fire on
// `fn _synth<const X: usize>()` const generic parameters
// synthesised by a proc-macro derive whose expansion attaches a
// user-source span to the const-generic identifier. Mirrors the
// clap-derive span shape on a minimal `#[synth_const_generic]`
// attribute.

#![allow(dead_code, unused_variables, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthConstGeneric;

#[derive(SynthConstGeneric)]
#[synth_const_generic]
struct UsesSynthConstGeneric;

fn main() {}
