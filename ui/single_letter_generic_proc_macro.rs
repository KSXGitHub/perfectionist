// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// aux-build:proc_macro_synth_binding.rs

// Regression test: `single_letter_generic` must not fire on
// `fn _synth<T>() { ... }` generic parameters synthesised by a
// proc-macro derive whose expansion attaches a user-source span to
// the generic identifier. The `SynthGeneric` derive imported below
// mirrors the `clap_derive` span shape on a minimal `#[synth_generic]`
// attribute.

#![allow(dead_code, unused_variables, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthGeneric;

#[derive(SynthGeneric)]
#[synth_generic]
struct UsesSynthGeneric;

fn main() {}
