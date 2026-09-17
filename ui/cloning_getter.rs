// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::ffi::{CString, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

// A field type that renders as a `String` but is not one, and is not
// `Copy` either.
#[derive(Clone)]
struct Badge(String);

impl fmt::Display for Badge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

struct Person {
    first_name: String,
    middle_name: Option<String>,
    home: PathBuf,
    tags: Vec<String>,
    age: u32,
    scores: [u8; 4],
    badge: Badge,
    token: String,
    clone_url: String,
    generated: String,
}

impl Person {
    // Bad: `clone` of a `String` field.
    fn first_name(&self) -> String {
        self.first_name.clone()
    }

    // Bad: `clone` of an `Option<String>` field.
    fn middle_name(&self) -> Option<String> {
        self.middle_name.clone()
    }

    // Bad: `to_path_buf` after a deref is still the field copied out.
    fn home(&self) -> PathBuf {
        self.home.to_path_buf()
    }

    // Bad: `to_vec`.
    fn tags(&self) -> Vec<String> {
        self.tags.to_vec()
    }

    // Bad: `to_string` on a `String` field is the field copied out.
    fn token(&self) -> String {
        self.token.to_string()
    }

    // Good: copies a field, but names none and is not `get_*`, so the
    // default list never calls it a getter.
    fn display_name(&self) -> String {
        self.first_name.to_string()
    }

    // Good: the borrowed forms.
    fn first_name_ref(&self) -> &str {
        &self.first_name
    }

    fn middle_name_ref(&self) -> Option<&str> {
        self.middle_name.as_deref()
    }

    fn home_ref(&self) -> &Path {
        &self.home
    }

    // Good: a `Copy` field returned by value is not a clone.
    fn age(&self) -> u32 {
        self.age
    }

    // Good: cloning a `Copy` field still returns it by value, which is
    // the borrowed form's equal.
    fn cloned_age(&self) -> u32 {
        self.age.clone()
    }

    // Good: `to_string` renders a `u32`; no borrow of `self.age` is a
    // `String`, so there is no borrowed form to ask for.
    fn age_label(&self) -> String {
        self.age.to_string()
    }

    // Good: `to_vec` on a `Copy` array produces a different type, so it
    // builds a value rather than copying the field out.
    fn scores(&self) -> Vec<u8> {
        self.scores.to_vec()
    }

    // Good: not a getter — the body does more than copy a field.
    fn shouted(&self) -> String {
        self.first_name.to_uppercase()
    }

    // Good: `to_string` renders `self.badge` rather than copying it.
    // `Badge` is not `Copy`, and no borrow of `self.badge` is a `String`,
    // so there is no borrowed form to ask for.
    fn badge(&self) -> String {
        self.badge.to_string()
    }

    // Good: `to_*` announces a costly conversion, so the copy is what
    // the name already promises.
    fn to_first_name(&self) -> String {
        self.first_name.clone()
    }

    // Good: `into_*` likewise announces that it costs something.
    #[expect(perfectionist::non_consuming_into_conversion, reason = "exercises cloning_getter")]
    fn into_first_name(&self) -> String {
        self.first_name.clone()
    }

    // Good: `as_*` promises the opposite, and has its own rule.
    #[expect(perfectionist::cloning_as_conversion, reason = "exercises cloning_getter")]
    fn as_first_name(&self) -> String {
        self.first_name.clone()
    }

    // Bad: `get_*` is the one name shape the default list calls a
    // getter outright, whatever it is named after.
    fn get_anything(&self) -> String {
        self.first_name.to_owned()
    }

    // Good: no entry of the default list covers these, and they name no
    // field, so nothing calls them getters.
    fn cloned_first_name(&self) -> String {
        self.first_name.clone()
    }

    fn clone_first_name(&self) -> String {
        self.first_name.clone()
    }

    fn unrelated(&self) -> String {
        self.first_name.clone()
    }

    // Bad: named for a field, which no configuration talks the rule out
    // of. The name says it copies, but so does the field it is named
    // after.
    fn clone_url(&self) -> String {
        self.clone_url.clone()
    }

    // Good: not a getter — it takes an argument, even though the body
    // copies a field and nothing else.
    fn first_name_or(&self, _fallback: &str) -> String {
        self.first_name.clone()
    }

    // Good: not a getter — it takes an argument.
    fn name_or(&self, fallback: &str) -> String {
        if self.first_name.is_empty() { fallback.to_owned() } else { self.first_name.clone() }
    }

    // Good: `&mut self` is a mutator, not a getter.
    fn take_name(&mut self) -> String {
        self.first_name.clone()
    }
}

struct Config;

struct Holder {
    label: OsString,
    cpath: CString,
    boxed: Box<str>,
    badge: Badge,
    handle: Arc<Config>,
    counted: Rc<Config>,
}

impl Holder {
    // Bad: `to_os_string`, whose borrowed form is `&OsStr`.
    fn label(&self) -> OsString {
        self.label.to_os_string()
    }

    // Bad: a `CString` borrows as `&CStr`.
    fn cpath(&self) -> CString {
        self.cpath.clone()
    }

    // Bad: a `Box<T>` is owned storage for one `T`, so what a caller
    // borrows is the `T` -- `&str`, never `&Box<str>`.
    fn boxed(&self) -> Box<str> {
        self.boxed.clone()
    }

    // Bad: a type with no borrowed form of its own falls back to `&T`.
    fn badge(&self) -> Badge {
        self.badge.clone()
    }

    // Good: cloning an `Arc` bumps a refcount rather than copying what
    // it points at, and a caller that keeps the handle needs to own
    // one, so `&Arc<Config>` would not serve.
    fn handle(&self) -> Arc<Config> {
        self.handle.clone()
    }

    // Good: an `Rc` for the same reason.
    fn counted(&self) -> Rc<Config> {
        self.counted.clone()
    }
}

struct Borrowed<'a> {
    name: &'a str,
}

impl<'a> Borrowed<'a> {
    // Good: the field is already a borrow, so the call copies nothing
    // and the method already returns the borrowed form.
    #[expect(noop_method_call, reason = "ui fixture")]
    fn name(&self) -> &'a str {
        self.name.clone()
    }
}

// Good: a trait fixes the signature.
trait Named {
    fn name(&self) -> String;
}

impl Named for Person {
    fn name(&self) -> String {
        self.first_name.clone()
    }
}

// Good: a method a macro expands to is not measured. The generated
// method is named for the field it returns, so the field-match clause
// would admit the name and only `Span::from_expansion` stops the
// diagnostic; a method named anything else would leave this passing
// whether the guard were there or not.
macro_rules! getter {
    ($name:ident, $field:ident) => {
        impl Person {
            fn $name(&self) -> String {
                self.$field.clone()
            }
        }
    };
}

getter!(generated, generated);

fn main() {}
