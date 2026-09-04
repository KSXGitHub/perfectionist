// aux-build:synth_core_path.rs

// Regression test: `core_instead_of_std` must not fire on a `core::`
// path synthesised by a proc-macro derive whose expansion stamps every
// token of the path with a user-source span. The rule's diagnostic span
// is one token of a longer path, so the `report_in_external_macro:
// false` filter sees user-written source and lets it through; the
// `hir_in_external_macro` guard on the enclosing item is what keeps the
// rule from offering to rewrite a path that exists nowhere in the
// user's source.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, dead_code, reason = "ui fixture")]

extern crate synth_core_path;

use synth_core_path::SynthCorePath;

#[derive(SynthCorePath)]
#[synth_core_path]
struct UsesSynthCorePath;

// Applied a second time to confirm the derive's synthesised anchor does
// not collide across uses in one crate.
#[derive(SynthCorePath)]
#[synth_core_path]
struct UsesSynthCorePathAgain;

fn main() {}
