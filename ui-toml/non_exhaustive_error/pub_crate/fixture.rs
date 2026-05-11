// Under `require_for = "pub_crate"`, an enum literally declared
// `pub(crate)` must be flagged. The `pub` enum is flagged for the
// same reason it is under the default mode. A `pub(super)` enum
// nested two modules deep has a `Restricted` scope below the crate
// root, so it stays silent — `"pub_crate"` does not promote
// stricter visibilities.

pub(crate) enum CrateLocalError {
    Variant,
}

pub enum AlsoFlaggedError {
    Variant,
}

mod outer {
    pub(super) mod inner {
        // `pub(super)` here points at `outer`, not at the crate
        // root, so the scope is `outer`'s module def-id — strictly
        // narrower than `pub(crate)`.
        pub(super) enum DeepError {
            Variant,
        }
    }
}

fn main() {}
