// aux-build:proc_macro_synth_binding.rs

// Regression test: `cloning_getter` must not fire on a
// `fn _synth(&self) -> String { self.s.clone() }` synthesised by a
// proc-macro derive whose expansion stamps the user's span on the whole
// generated `impl`, the way an accessor derive spans a generated getter
// over the field it reads. The rule reports at the method's `def_span`,
// so a user-spanned method slips past `report_in_external_macro: false`;
// and because the enclosing `impl` carries a user span too, the
// `hir_in_external_macro` guard the sibling late passes use has nothing
// to find either. The text-based `is_from_proc_macro` is what holds
// here. The `SynthCloningGetter` derive imported below builds that span
// shape on a minimal `#[synth_cloning_getter]` attribute.

#![allow(dead_code, unused, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthCloningGetter;

#[derive(SynthCloningGetter)]
#[synth_cloning_getter]
struct UsesSynthCloningGetter;

fn main() {}
