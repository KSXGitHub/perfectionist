// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

struct Person {
    name: String,
    home: PathBuf,
    tags: Vec<String>,
    nickname: Option<String>,
    parsed: Result<String, String>,
    raw: Vec<u8>,
    table: LazyLock<Vec<String>>,
    slot: Slot,
    age: u32,
}

struct Slot(String);

impl Deref for Slot {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl Person {
    // Bad: `to_` promises a conversion that costs something, and this
    // hands a field straight over.
    fn to_name(&self) -> &str {
        &self.name
    }

    // Bad: same, returning `&Path`.
    fn to_home(&self) -> &Path {
        &self.home
    }

    // Bad: same, returning a slice.
    fn to_tags(&self) -> &[String] {
        &self.tags
    }

    // Bad: dereferencing a `PathBuf` is free as well.
    fn to_home_path(&self) -> &Path {
        &*self.home
    }

    // Bad: an `as_*` call is free, which is the whole point of the
    // prefix this rule asks for.
    fn to_nickname(&self) -> Option<&str> {
        self.nickname.as_deref()
    }

    // Bad: a borrow under a `Result` is as free as one under an
    // `Option`, so the two shapes are measured alike.
    fn to_parsed(&self) -> Result<&String, &String> {
        self.parsed.as_ref()
    }

    // Good: a `to_` that hands back an owned value is the costly
    // conversion the prefix announces.
    fn to_owned_name(&self) -> String {
        self.name.clone()
    }

    // Good: an owned `Option` is not a borrow.
    fn to_nickname_owned(&self) -> Option<String> {
        self.nickname.clone()
    }

    // Good: a `Copy` value is owned, not borrowed.
    fn to_age(&self) -> u32 {
        self.age
    }

    // Good: the borrow costs a UTF-8 check. This is the shape the
    // guidelines name as `to_`'s own: `Path::to_str` has it, and
    // `as_str` would be the wrong name for it.
    fn to_text(&self) -> Option<&str> {
        self.home.to_str()
    }

    // Good: the scan is the cost, and paying one is what `to_`
    // announces.
    fn to_bytes(&self) -> &[u8] {
        let end = self.raw.iter().position(|byte| *byte == 0);
        &self.raw[..end.unwrap_or(self.raw.len())]
    }

    // Good: `LazyLock::deref` runs the initializer, so this borrow is
    // not free. Which `Deref` implementations only project is asked of
    // the field's type, since neither spelling of the deref shows it.
    fn to_table(&self) -> &[String] {
        &self.table
    }

    // Good: the same `LazyLock` deref written out. An explicit `*` is
    // recorded as a method call rather than as a coercion, so it is
    // asked about separately.
    fn to_table_explicit(&self) -> &[String] {
        &*self.table
    }

    // Not flagged: an under-approximation. `Slot::deref` only projects,
    // so this borrow is free and the rule's premise condemns it -- but a
    // `Deref` the rule cannot name may run anything, and counting an
    // unrecognised one as code is what keeps the case above unreported.
    fn to_slot(&self) -> &str {
        &self.slot
    }

    // Not flagged: an under-approximation. `split_first` costs nothing,
    // so the rule's own premise condemns this `to_*` -- but what the
    // rule reads is a borrow handed straight over, and a `match` is not
    // one. Missing it is the safe direction, since the alternative is
    // judging what an arbitrary body costs.
    fn to_tag(&self) -> Option<&String> {
        match self.tags.split_first() {
            Some((first, _)) => Some(first),
            None => None,
        }
    }

    // Not flagged: not the `to_` prefix. `as_*` is the prefix this
    // rule asks for.
    fn as_name(&self) -> &str {
        &self.name
    }

    // Not flagged: `token` begins with `to` but not with `to_`, which
    // is where the prefix test draws its line.
    fn token(&self) -> &str {
        &self.name
    }

    // Not flagged: takes an argument, so it is not a conversion of
    // `self`. Both halves hold otherwise, so this pins that the arity
    // requirement is what excludes it.
    fn to_name_or(&self, fallback: &str) -> &str {
        &self.name
    }

    // Not flagged: `&mut self` is not the receiver this measures.
    fn to_name_mut(&mut self) -> &mut String {
        &mut self.name
    }

    // Not flagged: what an `async fn` signature names is the opaque
    // future, not the borrow awaited out of it.
    async fn to_awaited_name(&self) -> &str {
        &self.name
    }

    // Not flagged: an explicitly typed receiver is `ImplicitSelfKind::None`
    // however it is spelled, so the eligibility test does not see the
    // `&self` this is equivalent to.
    fn to_spelled_out(self: &Self) -> &str {
        &self.name
    }
}

// Not flagged: a trait fixes the signature, so the impl cannot change it.
trait ToName {
    fn to_name(&self) -> &str;
}

impl ToName for Person {
    fn to_name(&self) -> &str {
        &self.name
    }
}

fn main() {}
