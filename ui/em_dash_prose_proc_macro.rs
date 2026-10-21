// aux-build:proc_macro_synth_binding.rs

// Regression test: `em_dash_prose` must not fire on a panic message
// synthesised by a proc-macro derive whose expansion attaches a
// user-source span to the message literal. That span shape is what
// slips past rustc's `report_in_external_macro: false` check, and the
// rule stays silent for a different reason: it reads the span's own
// source text, which is the attribute identifier rather than a string
// literal, so there is no message to scan.

#![allow(dead_code, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::{SynthDocComment, SynthExpectMessage};

#[derive(SynthExpectMessage)]
#[synth_expect_message]
struct UsesSynthExpectMessage;

// A derive can emit a `#[doc]` attribute, which is what a `///` line
// lowers to, so a doc comment with a dash in it is a thing a derive can
// produce. The rule does not fire on a synthesised doc comment any more
// than on a synthesised message, for a reason of its own: both comment
// surfaces read the text of the files backing this crate's modules, and
// an attribute the expansion carries is in no such file.
#[derive(SynthDocComment)]
#[synth_doc_comment]
struct UsesSynthDocComment;

fn main() {}
