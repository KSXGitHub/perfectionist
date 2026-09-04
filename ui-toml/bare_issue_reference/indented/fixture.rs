// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
struct Foo;

impl Foo {
    /// Bad: this doc block is indented inside an `impl`. The reference
    /// sits on the continuation line: closes #51 — which earlier was
    /// silently skipped. The appended `[#51]: URL` definition reuses
    /// the indented `///` prefix.
    fn bar() {}
}

fn main() {}
