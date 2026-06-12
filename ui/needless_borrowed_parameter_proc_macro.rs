// aux-build:proc_macro_synth_binding.rs

// Regression test: `needless_borrowed_parameter` must not fire on a
// `fn _synth(item: &str) -> String { item.to_owned() }` synthesised by
// a proc-macro derive whose expansion attaches a user-source span to
// the parameter *type*. The rule reports at the parameter-type span, so
// without the `hir_in_external_macro` guard the user-span `&str` slips
// past `report_in_external_macro: false`. The `SynthOwnedParam` derive
// imported below mirrors the `clap_derive` span shape on a minimal
// `#[synth_owned_param]` attribute.

#![allow(dead_code, unused_variables, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthOwnedParam;

#[derive(SynthOwnedParam)]
#[synth_owned_param]
struct UsesSynthOwnedParam;

fn main() {}
