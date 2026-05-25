// aux-build:proc_macro_synth_binding.rs

// Regression test: `single_letter_static_item` must not fire on
// `static X: u32 = 1;` items synthesised by a proc-macro derive
// whose expansion attaches a user-source span to the static
// identifier. Mirrors the clap-derive span shape on a minimal
// `#[synth_static_item]` attribute.

#![allow(dead_code, unused_variables, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthStaticItem;

#[derive(SynthStaticItem)]
#[synth_static_item]
struct UsesSynthStaticItem;

fn main() {}
