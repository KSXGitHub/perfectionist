// The `emit_missing_field_no_sugg` fallback: the missing-`reason`
// finding emitted without a code suggestion, because the attribute's
// source snippet cannot be recovered.
//
// The `allow $lints` invocation is assembled from two source files —
// the `allow` key from this file's macro definition, the lint list
// from the `include!`d call site — so the meta item's span begins in
// one file and ends in another, and `span_to_snippet` fails on it.
// The rule still reports the missing `reason`, but points at the fix
// in prose instead of splicing one in blind.
//
// The call site lives under `auxiliary/`, the one directory
// compiletest skips when collecting tests; any other `.rs` file beside
// this one would be picked up as a fixture in its own right.

macro_rules! silence {
    ($lints:tt) => {
        // Bad: no `reason`. `dead_code` is on `allow_attributes`'
        // exempt list, so that sibling rule stays quiet and this
        // fixture pins one diagnostic.
        #[cfg_attr(all(), allow $lints)]
        fn _silenced_across_files() {}
    };
}

include!("auxiliary/call_site.rs");

fn main() {}
