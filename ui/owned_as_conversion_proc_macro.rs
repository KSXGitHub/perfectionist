// aux-build:proc_macro_synth_binding.rs

// Regression test: `owned_as_conversion` must not fire on a
// `fn as_s(&self) -> String { self.s.clone() }` synthesised by a
// proc-macro derive whose expansion stamps the user's span on the whole
// generated `impl`, the way an accessor derive spans a generated method
// over the field it reads. The rule reports at the method's `def_span`,
// so a user-spanned method slips past `report_in_external_macro: false`;
// and because the enclosing `impl` carries a user span too, the
// `hir_in_external_macro` guard the sibling late passes use has nothing
// to find either. The text-based `is_from_proc_macro` is what holds
// here. The `SynthOwnedAsConversion` derive imported below builds that
// span shape on a minimal `#[synth_owned_as_conversion]` attribute.
//
// The synthesised method wears the `as_` prefix and copies a field, so
// both of the rule's shapes admit it and the guard is the only thing
// left stopping the diagnostic. A method named anything else would
// leave the fixture passing whether the guard were there or not.

#![allow(dead_code, unused, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthOwnedAsConversion;

#[derive(SynthOwnedAsConversion)]
#[synth_owned_as_conversion]
struct UsesSynthOwnedAsConversion;

fn main() {}
