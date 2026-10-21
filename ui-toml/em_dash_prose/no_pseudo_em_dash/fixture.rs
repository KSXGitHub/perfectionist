// `flag_pseudo_em_dash = false` leaves a spaced hyphen run alone.
// U+2014 has no knob of its own and is reported either way.

// Bad: an em dash is reported whatever the knobs say.
// Wrote the report — 3 warnings remain.

// Not flagged: a spaced hyphen run is left alone.
// Loaded the manifest -- 12 entries were skipped.

fn main() {}
