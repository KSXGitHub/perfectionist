// `flag_en_dash = false` leaves U+2013 alone. U+2014 has no knob of
// its own and is reported either way.

// Bad: an em dash — is reported whatever the knobs say.
// Good: an en dash – is left alone.

fn main() {}
