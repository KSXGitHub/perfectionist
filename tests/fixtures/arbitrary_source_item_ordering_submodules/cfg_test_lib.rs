#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, unused_imports, dead_code, reason = "fixture")]

pub struct Thing;

mod separate;

#[cfg(test)]
mod tests {
    pub use super::Thing;

    // Live only under the unit-test target. In a library-only build this
    // module is configured out, so the rule must not reach it.
    pub mod helpers {}
}
