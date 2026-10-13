// aux-build:proc_macro_synth_binding.rs

// Regression test: `some_bool_comparison` must not fire on a
// `FLAG == Some(true)` a proc-macro derive synthesised, where the
// expansion stamps the user's span on every token of the comparison the
// way `quote_spanned!` over a field's span does. The comparison then
// reads as user-written, so neither `report_in_external_macro: false`
// nor the rule's own `Span::from_expansion` bail has anything to find,
// and `hir_in_external_macro` -- which also reads the enclosing item's
// `def_span`, left at the derive's call site -- is the only thing
// stopping the diagnostic. The comparison is one the rule fires on
// wherever it is hand-written, so without that guard the fixture would
// turn red.

#![allow(dead_code, unused, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthSomeBoolComparison;

const FLAG: Option<bool> = Some(true);

#[derive(SynthSomeBoolComparison)]
#[synth_some_bool_comparison]
struct UsesSynthSomeBoolComparison;

fn main() {}
