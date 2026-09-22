// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Clone)]
struct Widget(u8);

struct Person {
    name: String,
    home: PathBuf,
    tags: Vec<String>,
    age: u32,
    label: Box<str>,
    middle: Option<String>,
    cpath: CString,
    shared: Rc<String>,
    widget: Widget,
}

impl Person {
    // Bad: `as_` promises free, this copies a field.
    fn as_name(&self) -> String {
        self.name.clone()
    }

    // Bad: `to_path_buf` is a copying method too, and a `PathBuf`
    // borrows as `&Path`.
    fn as_home(&self) -> PathBuf {
        self.home.to_path_buf()
    }

    // Bad: `to_vec` is a copying method, and a `Vec<T>` borrows as
    // `&[T]`.
    fn as_tags(&self) -> Vec<String> {
        self.tags.to_vec()
    }

    // Bad: a `Box<T>` is owned storage for one `T`, so what a caller
    // borrows is the `T` itself: `&str`, never `&Box<str>`.
    fn as_label(&self) -> Box<str> {
        self.label.clone()
    }

    // Bad: the borrowed form under an `Option` is the inner type's own.
    fn as_middle(&self) -> Option<String> {
        self.middle.clone()
    }

    // Bad: a `CString` borrows as `&CStr`.
    fn as_cpath(&self) -> CString {
        self.cpath.clone()
    }

    // Bad: `Widget` is on no list of owning types, so the return type
    // says nothing. The body does: it copies a field whose type is not
    // `Copy`, and a borrow of that field would have served.
    fn as_widget(&self) -> Widget {
        self.widget.clone()
    }

    // Bad: `to_string` renders a `u32` rather than copying the field,
    // so no borrow of `self.age` is a `String` and there is no borrowed
    // form to ask for. The return type is owned all the same, and the
    // rename is the whole fix.
    fn as_age_label(&self) -> String {
        self.age.to_string()
    }

    // Bad: the `Vec` is built rather than copied out, which the body
    // shape cannot see and the return type can.
    fn as_words(&self) -> Vec<String> {
        self.name.split(' ').map(str::to_owned).collect()
    }

    // Bad: a map the caller must drop, under a prefix that promises a
    // view.
    fn as_index(&self) -> HashMap<String, u32> {
        HashMap::new()
    }

    // Bad: `String::new` allocates nothing, so this pins that the rule
    // reports ownership rather than allocation. The caller still
    // receives a value of their own where a borrow was promised.
    fn as_key(&self) -> String {
        String::new()
    }

    // Bad: an owned value under a `Result` is still owned; the error
    // type is the failure path rather than the conversion's product.
    fn as_parsed(&self) -> Result<String, ()> {
        Ok(self.name.clone())
    }

    // Good: returning `&str` costs nothing, which is what `as_` says.
    fn as_name_ref(&self) -> &str {
        &self.name
    }

    // Good: returning `&Path` costs nothing, which is what `as_` says.
    fn as_home_ref(&self) -> &Path {
        &self.home
    }

    // Good: a borrow under an `Option` is still a borrow, so the
    // recursion into the payload has to reach the reference and stop.
    fn as_middle_ref(&self) -> Option<&String> {
        self.middle.as_ref()
    }

    // Good: `Cow` is the honest type for a conversion that is
    // sometimes free, and the lifetime it carries is what tells the
    // rule the value may be borrowed from the receiver.
    fn as_display_name(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.name)
    }

    // Good: handing a field back without copying it is not the shape
    // this rule reads at all, since there is no call to be free or
    // costly.
    fn as_age(&self) -> u32 {
        self.age
    }

    // Not flagged: this is the `Copy` case. The body *is* the shape,
    // and only the field being `Copy` keeps it quiet, so removing that
    // exemption is what this pins. The clone itself is not a form to
    // imitate; `clippy::clone_on_copy` is what reports it.
    fn as_cloned_age(&self) -> u32 {
        self.age.clone()
    }

    // Good: cloning an `Rc` bumps a refcount rather than copying what it
    // points at, and a caller that keeps the handle has to own one.
    fn as_shared(&self) -> Rc<String> {
        self.shared.clone()
    }

    // Not flagged: takes `self` by value, so the receiver excludes it
    // before either shape is read. An `as_*` that consumes is its own
    // mistake, but not this rule's.
    fn as_owned_name(self) -> String {
        self.name.clone()
    }

    // Not flagged: not the `as_` prefix. `to_*` announces a conversion
    // that costs something, so the owned return is what that name
    // promises.
    fn to_name(&self) -> String {
        self.name.clone()
    }

    // Not flagged: `ascii_name` begins with `as` but not with `as_`,
    // which is where the prefix test draws its line.
    fn ascii_name(&self) -> String {
        self.name.clone()
    }

    // Not flagged: takes an argument, so it is not a conversion of
    // `self`. Both shapes hold otherwise, so this pins that the arity
    // requirement is what excludes it.
    fn as_name_or(&self, fallback: &str) -> String {
        self.name.clone()
    }

    // Not flagged: `&mut self` is not the receiver this measures.
    fn as_taken_name(&mut self) -> String {
        self.name.clone()
    }
}

// Not flagged: a trait fixes the signature, so the impl cannot change it.
trait AsName {
    fn as_name(&self) -> String;
}

impl AsName for Person {
    fn as_name(&self) -> String {
        self.name.clone()
    }
}

fn main() {}
