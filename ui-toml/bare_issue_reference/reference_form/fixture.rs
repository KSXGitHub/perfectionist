/// Bad: closes #99 — under `form = "reference"`, the suggestion
/// substitutes `[#99]` (and the author adds the matching
/// `[#99]: URL` definition by hand). Applicability is
/// `MaybeIncorrect` because the unaccompanied bracket form is
/// not a complete markdown link on its own.
fn _doc_reference_form() {}

fn main() {}
