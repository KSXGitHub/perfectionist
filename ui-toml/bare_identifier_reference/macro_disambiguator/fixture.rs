// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![allow(dead_code, reason = "ui fixture")]

#[macro_export]
macro_rules! dual {
    () => {};
}

fn dual() {}

/// A public macro and a private function both answer to `dual`; the
/// disambiguator the help suggests must be `macro@` (the eligible public
/// target), not `value@` (the private function) or `type@` (absent).
pub fn refer_macro() {}

fn main() {}
