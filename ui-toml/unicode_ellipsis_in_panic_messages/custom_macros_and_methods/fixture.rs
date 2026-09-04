// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(dead_code, reason = "ui fixture")]
#![warn(perfectionist::unicode_ellipsis_in_panic_messages)]

// Both knob-pairs exercised in one fixture:
// - `extra_macros = ["my_panic"]` + `ignore_macros = ["panic"]`
// - `extra_methods = ["expect_with"]` + `ignore_methods = ["expect"]`
// After the merge `panic` / `expect` are out and `my_panic` /
// `expect_with` are in.

macro_rules! my_panic {
    ($msg:literal) => {
        panic!($msg)
    };
}

trait OptionExt<Inner> {
    fn expect_with(self, message: &str) -> Inner;
}

impl<Inner> OptionExt<Inner> for Option<Inner> {
    fn expect_with(self, message: &str) -> Inner {
        self.expect(message)
    }
}

fn _macros() {
    // `panic` dropped via `ignore_macros`: stays quiet.
    panic!("legacy panic …");
    // `my_panic` added via `extra_macros`: fires.
    my_panic!("project-specific panic …");
}

fn _methods() {
    // `expect` dropped via `ignore_methods`: stays quiet.
    let _: i32 = Some(1).expect("legacy expect …");
    // `expect_with` added via `extra_methods`: fires.
    let _: i32 = Some(1).expect_with("project-specific expect …");
}

fn main() {}
