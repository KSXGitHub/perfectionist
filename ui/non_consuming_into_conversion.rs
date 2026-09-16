// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

struct Person {
    name: String,
    age: u32,
}

impl Person {
    // Bad: `into_` promises to consume, this copies.
    fn into_name(&self) -> String {
        self.name.clone()
    }

    // Bad: the returned borrow is tied to the `&self` receiver.
    fn into_name_ref(&self) -> &str {
        &self.name
    }

    // Good: moves the field out of a consumed `self`.
    fn into_owned_name(self) -> String {
        self.name
    }

    // Good: a `Copy` field handed back by value is the caller's own.
    fn into_age(&self) -> u32 {
        self.age
    }

    // Good: cloning a `Copy` field still yields a value of their own.
    fn into_cloned_age(&self) -> u32 {
        self.age.clone()
    }

    // Not flagged: not the `into_` prefix.
    fn internal_name(&self) -> String {
        self.name.clone()
    }

    // Not flagged: `into` without the underscore is not the prefix,
    // which pins the boundary.
    fn intonation(&self) -> String {
        self.name.clone()
    }
}

// The lifetime case the rule has to get right: `Borrowed<'a>` already
// carries `'a`, so a returned `&'a str` outlives the `&self` borrow and
// is not tied to it.
struct Borrowed<'a> {
    name: &'a str,
    owned: String,
}

impl<'a> Borrowed<'a> {
    // Good: `'a` is the type's own lifetime, not the receiver's borrow.
    fn into_name(&self) -> &'a str {
        self.name
    }

    // Bad: this one IS tied to the receiver — elided to `&'_ self`.
    fn into_owned_ref(&self) -> &str {
        &self.owned
    }

    // Good: consumes `self` and hands back the type's own borrow.
    fn into_consumed_name(self) -> &'a str {
        self.name
    }
}

fn main() {}
