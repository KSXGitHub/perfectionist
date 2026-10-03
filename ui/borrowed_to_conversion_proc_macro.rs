// aux-build:proc_macro_synth_binding.rs

// Regression test: `borrowed_to_conversion` must not fire on a
// `fn to_s(&self) -> &str { &self.s }` synthesised by a proc-macro
// derive whose expansion stamps the user's span on the whole generated
// `impl`, the way an accessor derive spans a generated method over the
// field it reads. The rule reports at the method's `def_span`, so a
// user-spanned method slips past `report_in_external_macro: false`; and
// because the enclosing `impl` carries a user span too, the
// `hir_in_external_macro` guard the sibling late passes use has nothing
// to find either. The text-based `is_from_proc_macro` is what holds
// here. The `SynthBorrowedToConversion` derive imported below builds
// that span shape on a minimal `#[synth_borrowed_to_conversion]`
// attribute.
//
// The synthesised method wears the `to_` prefix and hands a field
// straight back as a borrow, so every half of the trigger admits it and
// the guard is the only thing left stopping the diagnostic. A method
// named anything else, or one doing any work on the way to the borrow,
// would leave the fixture passing whether the guard were there or not.

#![allow(dead_code, unused, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthBorrowedToConversion;

#[derive(SynthBorrowedToConversion)]
#[synth_borrowed_to_conversion]
struct UsesSynthBorrowedToConversion;

fn main() {}
