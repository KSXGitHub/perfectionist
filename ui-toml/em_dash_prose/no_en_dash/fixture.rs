// `flag_en_dash = false` leaves U+2013 alone. U+2014 has no knob of
// its own and is reported either way.

// Bad: an em dash is reported whatever the knobs say.
// Wrote the report — 3 warnings remain.

// Not flagged: an en dash is left alone.
// Retrying in a moment – the server was busy.

fn main() {}
