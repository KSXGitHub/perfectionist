// aux-build:proc_macro_synth_binding.rs

// Regression test: `map_or_usage` must not fire on a
// `FLAG.map_or(false, |flag| flag)` synthesised by a proc-macro derive
// whose expansion stamps the user's span on every token of the call.
// The rule reports at the method name and its arguments, so a
// user-spanned call slips past both `Span::from_expansion` and
// `report_in_external_macro: false`; the enclosing item's `def_span`
// is what still says where the call lives, and reading it is what the
// `hir_in_external_macro` guard does.
//
// The synthesised call is one the rule fires on when a human writes it
// — an `Option` receiver and a `false` default, which is the
// `is_some_and` shape — so the guard is the only thing stopping the
// diagnostic. A call on another receiver type, or a `map_or_default`,
// would leave the fixture passing whether the guard were there or not.

#![allow(dead_code, unused, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthMapOrUsage;

const FLAG: Option<bool> = Some(true);

#[derive(SynthMapOrUsage)]
#[synth_map_or_usage]
struct UsesSynthMapOrUsage;

fn main() {}
