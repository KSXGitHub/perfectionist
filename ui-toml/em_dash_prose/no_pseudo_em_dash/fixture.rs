// `flag_pseudo_em_dash = false` leaves a spaced hyphen run alone.
// U+2014 has no knob of its own and is reported either way.

// Bad: an em dash — is reported whatever the knobs say.
// Not flagged: ASCII hyphens -- are left alone.

fn main() {}
