// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(dead_code, reason = "ui fixture")]
#![warn(perfectionist::single_letter_closure_param)]

// `extra_trivial_callback_methods = ["when"]` adds to the built-in
// list and `ignore_trivial_callback_methods = ["sort_by"]` drops the
// default back out. The closure bodies below are single expressions
// that are NOT trivial wrappers (`a.0.cmp(&b.0)` accesses a field
// rather than the parameter itself), so the trivial-callback method
// allowlist is the only thing keeping them quiet.

struct Builder;
impl Builder {
    fn when<Cmp: FnMut(&(u32, u32), &(u32, u32)) -> std::cmp::Ordering>(&self, _: Cmp) {}
}

fn main() {
    let mut pairs: Vec<(u32, u32)> = vec![(3, 0), (1, 0), (2, 0)];

    // `sort_by` dropped from the trivial-callback allowlist via
    // `ignore_trivial_callback_methods`: `a` and `b` now fire.
    pairs.sort_by(|a, b| a.0.cmp(&b.0));

    // `when` added to the trivial-callback allowlist via
    // `extra_trivial_callback_methods`: `a` and `b` stay quiet.
    Builder.when(|a, b| a.0.cmp(&b.0));
}
