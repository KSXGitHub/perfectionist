// aux-build:proc_macro_synth_binding.rs

// Regression test for issue #410 (parallel-disk-usage):
// `single_letter_let_binding` must not fire on bindings synthesised
// by a proc-macro derive whose expansion attaches a user-source span
// to the binding identifier. `clap_derive` does this when expanding
// `#[clap(default_value_t = ...)]` into a `ToString`-based fallback;
// the `SynthBinding` derive imported below mirrors the same span
// shape on a minimal `#[synth_default]` attribute.

#![allow(dead_code, unused_variables)]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthBinding;

// The derive emits `const _: () = { let s = 1; let _ = s; };` with the
// `s` identifier carrying the span of `synth_default`. Without the
// proc-macro-expansion check, `perfectionist::single_letter_let_binding`
// would flag this `s` at the `synth_default` attribute.
#[derive(SynthBinding)]
#[synth_default = 1]
struct UsesSynthDefault;

fn main() {}
