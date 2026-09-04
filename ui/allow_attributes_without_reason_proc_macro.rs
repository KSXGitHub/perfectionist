// aux-build:proc_macro_synth_binding.rs

// Regression test: `allow_attributes_without_reason` must not fire on an
// `#[allow(dead_code)]` attribute synthesised by a proc-macro derive
// whose expansion attaches a user-source span to the attribute.
// Mirrors `clap_derive`'s generated `#[allow(...)]` shape, which
// carries the call-site span of a user `#[clap(...)]` attribute and so
// slips past both `Span::from_expansion` and the rule's
// `report_in_external_macro: false` filter.
// See <https://github.com/KSXGitHub/parallel-disk-usage/issues/430>.

#![allow(dead_code, unused_variables, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthSilenceReason;

#[derive(SynthSilenceReason)]
#[synth_silence_reason]
struct UsesSynthSilenceReason;

// Applied a second time to confirm the derive's synthesised anchor does
// not collide across uses in one crate.
#[derive(SynthSilenceReason)]
#[synth_silence_reason]
struct UsesSynthSilenceReasonAgain;

fn main() {}
