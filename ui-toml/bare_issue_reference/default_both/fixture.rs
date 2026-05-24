/// Bad: closes #77 — the default `suggestion_mode = "both"` offers
/// two `MaybeIncorrect` suggestions: one `/issues/77` URL and one
/// `/pull/77` URL, since a bare `#77` could name either.
fn _doc_default_both() {}

fn main() {}
