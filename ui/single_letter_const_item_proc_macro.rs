// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// aux-build:proc_macro_synth_binding.rs

// Regression test: `single_letter_const_item` must not fire on
// `const X: u32 = 1;` items synthesised by a proc-macro derive
// whose expansion attaches a user-source span to the const
// identifier. Mirrors the clap-derive span shape on a minimal
// `#[synth_const_item]` attribute.

#![allow(dead_code, unused_variables, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthConstItem;

#[derive(SynthConstItem)]
#[synth_const_item]
struct UsesSynthConstItem;

fn main() {}
