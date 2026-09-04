// The `emit_split_without_fix` fallback: a mixed `#[allow]` flagged
// with prose only, because the source text the structured suggestions
// need cannot be recovered.
//
// `reason = $reason` is assembled from two source files — the `reason`
// key from this file's macro definition, the literal from the
// `include!`d call site — so the meta item's span begins in one file
// and ends in another, and `span_to_snippet` fails on it. Rather than
// risk a rewrite that silently drops the reason, the rule declines the
// autofix and describes both resolutions in prose.
//
// The call site lives under `auxiliary/`, the one directory
// compiletest skips when collecting tests; any other `.rs` file beside
// this one would be picked up as a fixture in its own right.

macro_rules! silence {
    ($reason:literal) => {
        // Bad: `dead_code` is exempt (kept) while `non_snake_case` is
        // rewriteable. The mix is what routes the finding through the
        // split path rather than the whole-attribute one.
        #[allow(dead_code, non_snake_case, reason = $reason)]
        fn SilencedAcrossFiles() {}
    };
}

include!("auxiliary/call_site.rs");

fn main() {}
