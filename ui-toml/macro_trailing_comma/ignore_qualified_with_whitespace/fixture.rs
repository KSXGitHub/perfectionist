// Skipped: `std::vec!` is on the built-in name-based list (via
// final-segment matching against `"vec"`), but the fixture's
// `dylint.toml` adds the multi-segment, whitespace-padded entry
// `"  std::vec  "` to `ignore`. This exercises two things at once:
//
// 1. Multi-segment `ignore` entries. Before this fixture, only
//    `extra_macros`'s multi-segment branch was covered.
// 2. `parse_path`'s per-segment whitespace trimming, so a stray
//    space in the config doesn't silently disable the entry.
//
// The rule must NOT fire on the multi-line invocation below.

fn main() {
    let _ = std::vec![
        1,
        2,
        3
    ];
}
