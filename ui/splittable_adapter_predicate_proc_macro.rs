// aux-build:proc_macro_synth_binding.rs
//
// Regression test: the rule must not fire on a predicate a proc-macro
// derive synthesised, where the suggestion would be advice about code
// the author never wrote.
//
// The derive stamps every token it emits, the enclosing `fn` included,
// with the driving attribute's span, which leaves both span-reading
// guards nothing to find. `is_from_proc_macro` reads the source text
// under the span instead, and is what keeps this fixture silent.

#![allow(dead_code, unused, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthSplittableAdapterPredicate;

const VARS: &[&str] = &[" a ", " b "];

#[derive(SynthSplittableAdapterPredicate)]
#[synth_splittable_adapter_predicate]
struct UsesSynthSplittableAdapterPredicate;

fn main() {}
