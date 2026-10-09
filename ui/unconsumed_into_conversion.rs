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
    // Not flagged: `&mut self` is not the receiver this measures.
    fn into_mut_name(&mut self) -> String {
        self.name.clone()
    }

    // Not flagged: an explicitly typed receiver is `ImplicitSelfKind::None`
    // however it is spelled, so the eligibility test does not see the
    // `&self` this is equivalent to.
    fn into_spelled_out(self: &Self) -> String {
        self.name.clone()
    }

    // Not flagged: takes an argument, so it is converting something more
    // than `self`. The body is a field copy, so this pins that the arity
    // requirement is what excludes it.
    fn into_name_or(&self, fallback: &str) -> String {
        self.name.clone()
    }

    // Not flagged: what an `async fn` signature names is the opaque
    // future, which carries the receiver's lifetime however the body
    // behaves -- here it neither borrows nor copies.
    async fn into_awaited_age(&self) -> u32 {
        self.age
    }

    // Not flagged: an `impl Trait` signature names an opaque type that
    // carries the receiver's lifetime, so the rule reads neither that
    // lifetime nor the body behind it.
    fn into_opaque_label(&self) -> impl AsRef<str> {
        self.name.clone()
    }

    // Bad: a trait object's own lifetime is the receiver's, which `+ '_`
    // writes out, so this really does hand back a borrow of `self`.
    fn into_boxed_borrowing(&self) -> Box<dyn Fn(&str) -> usize + '_> {
        Box::new(str::len)
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

struct Owning {
    name: String,
}

impl Owning {
    // Good: a boxed closure owns everything it holds. The `&str` in its
    // signature is bound by the closure's own `for<'x>`, not by this
    // method's binder, so it is not a borrow of the receiver.
    fn into_callback(&self) -> Box<dyn Fn(&str) -> usize> {
        Box::new(str::len)
    }

    // Good: the `&str` in a function pointer's signature is bound by
    // that pointer's own `for<'x>`, not by this method's binder.
    fn into_fn_ptr(&self) -> fn(&str) -> usize {
        str::len
    }

    // Good: a `Vec` of function pointers is owned, and the `&u8` each
    // one takes is bound by that pointer's own `for<'x>`.
    fn into_fns(&self) -> Vec<fn(&u8) -> u8> {
        Vec::new()
    }
}

// Not flagged: a trait fixes the signature, so the impl cannot change it.
trait IntoName {
    fn into_name(&self) -> String;
}

impl IntoName for Person {
    fn into_name(&self) -> String {
        self.name.clone()
    }
}

fn main() {}
