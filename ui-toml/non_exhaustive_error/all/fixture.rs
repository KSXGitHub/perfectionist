// Under `require_for = "all"`, every error-shaped enum is flagged
// regardless of visibility, including module-private ones.

pub enum PublicError {
    Variant,
}

pub(crate) enum CrateLocalError {
    Variant,
}

enum PrivateError {
    Variant,
}

fn main() {}
