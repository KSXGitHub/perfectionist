// aux-build:proc_macro_synth_binding.rs

// Regression test: `prefer_expect_over_allow` must not fire on an
// `#[allow(non_snake_case)]` attribute synthesised by a proc-macro derive
// whose expansion stamps it with a user-source span. Mirrors `clap_derive`'s
// generated `#[allow(...)]` shape, which carries the call-site span of a
// user attribute and so slips past both `Span::from_expansion` and the
// rule's `report_in_external_macro: false` filter; the `is_from_proc_macro`
// guard is what keeps the rule from rewriting a suppression the user never
// wrote. See <https://github.com/KSXGitHub/parallel-disk-usage/issues/430>.

#![allow(unknown_lints, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthAllowRewriteable;

#[derive(SynthAllowRewriteable)]
#[synth_allow_rewriteable]
struct UsesSynthAllowRewriteable;

// Applied a second time to confirm the derive's synthesised anchor does
// not collide across uses in one crate.
#[derive(SynthAllowRewriteable)]
#[synth_allow_rewriteable]
struct UsesSynthAllowRewriteableAgain;

fn main() {}
