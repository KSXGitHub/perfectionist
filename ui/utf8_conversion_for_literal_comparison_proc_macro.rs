// aux-build:proc_macro_synth_binding.rs

// Regression test: the rule must not fire on a comparison a proc-macro
// derive synthesised. `SynthUtf8Comparison` stamps the user's span on
// every token it emits, the way a derive reaching for `quote_spanned!`
// over a field's span does, so the conversion reads as hand-written:
// its span carries no expansion for `report_in_external_macro: false`
// or the rule's own `Span::from_expansion` test to find. What still
// says where the conversion lives is the enclosing item's `def_span`,
// left at the derive's call site, and `hir_in_external_macro` is the
// only thing that reads it.
//
// The comparison is one the rule reports wherever it is hand-written,
// so this fixture turns red the moment that guard goes.

#![allow(dead_code, unused, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthUtf8Comparison;

const BYTES: &[u8] = b"true";

#[derive(SynthUtf8Comparison)]
#[synth_utf8_comparison]
struct UsesSynthUtf8Comparison;

fn main() {}
