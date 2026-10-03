// aux-build:proc_macro_synth_binding.rs
//
// Regression test: the rule must not fire on a closure a proc-macro
// derive synthesised, where the suggestion would be advice about code
// the author never wrote.
//
// The derive stamps every token it emits, the enclosing `fn` included,
// with the driving attribute's span, which leaves both span-reading
// guards nothing to find. `is_from_proc_macro` reads the source text
// under the span instead, and is what keeps this fixture silent.

#![allow(dead_code, unused, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthSplittableAdapterStepChain;

const VARS: &[&str] = &[" a ", " b "];

#[derive(SynthSplittableAdapterStepChain)]
#[synth_splittable_adapter_step_chain]
struct UsesSynthSplittableAdapterStepChain;

fn main() {}
