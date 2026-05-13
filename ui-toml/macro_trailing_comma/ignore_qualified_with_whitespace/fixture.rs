// Skipped: `std::vec!` is on the built-in name-based list (via
// final-segment matching against `"vec"`), but the fixture's
// `dylint.toml` adds the multi-segment, whitespace-padded entry
// `"  std::vec  "` to `ignore`. This exercises two things at once:
//
// 1. Multi-segment `ignore` entries — only `name_based_extra`'s
//    multi-segment branch was previously covered.
// 2. `parse_path`'s per-segment whitespace trimming, so a stray
//    space in the config doesn't silently de-fang the entry.
//
// The rule must NOT fire on the multi-line invocation below.

fn main() {
    let _ = std::vec![
        1,
        2,
        3
    ];
}
