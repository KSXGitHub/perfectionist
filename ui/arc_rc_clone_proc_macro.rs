// aux-build:proc_macro_synth_binding.rs

// Regression test: `arc_rc_clone` must not fire on `arc.clone()`
// method calls synthesised by a proc-macro derive whose expansion
// attaches a user-source span to the method-call segment. The
// `SynthArcClone` derive imported below mirrors the `clap_derive`
// span shape on a minimal `#[synth_arc]` attribute.

#![allow(dead_code, unused_variables, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthArcClone;

#[derive(SynthArcClone)]
#[synth_arc]
struct UsesSynthArcClone;

fn main() {}
