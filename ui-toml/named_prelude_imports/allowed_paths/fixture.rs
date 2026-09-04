// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, dead_code, unused_imports, reason = "ui fixture")]

pub mod thing {
    pub struct A;
    pub struct B;
}

pub mod prelude {
    pub use crate::thing::A;
}

pub mod other {
    pub mod prelude {
        pub use crate::thing::B;
    }
}

// Not flagged: `crate::prelude` is in `allowed_paths`.
mod allowed {
    use crate::prelude::A;
}

// Flagged: `crate::other::prelude` is a different prelude, not exempt.
mod still_flagged {
    use crate::other::prelude::B;
}

fn main() {}
