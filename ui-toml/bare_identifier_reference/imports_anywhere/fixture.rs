#![allow(dead_code, reason = "ui fixture")]

pub struct RootItem;

pub mod sub {
    pub mod inner {
        pub struct InternalItem;
    }

    use crate::RootItem;
    use crate::sub::inner::InternalItem;
    use std::collections::HashMap;

    pub struct Defined;

    /// Mentions `Defined` (local), `InternalItem` (this module's
    /// subtree), `RootItem` (reached via `crate::`), and `HashMap`
    /// (another crate).
    pub fn doc(_: &HashMap<RootItem, InternalItem>, _: Defined) {}
}

fn main() {}
