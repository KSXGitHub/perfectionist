// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![allow(dead_code, reason = "ui fixture")]

pub struct Flagged;

pub struct Skipped;

/// Mentions `Flagged`, which still fires, and `Skipped`, which the
/// `skip_idents` list silences.
pub fn doc() {}

fn main() {}
