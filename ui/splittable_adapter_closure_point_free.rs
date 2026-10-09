// edition:2024
//
// How a lifted step is written once it has an adapter of its own. A step
// keeps its own name and takes the path of the type it was applied to, so
// the type a step was handed is what decides the path, and a primitive
// answers under its own name where a struct answers under the struct's.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::path::{Path, PathBuf};

/// Takes a `&Path`, which a `&PathBuf` reaches only through a deref.
fn canonical(path: &Path) -> Result<PathBuf, ()> {
    match path.is_absolute() {
        true => Ok(path.to_path_buf()),
        false => Err(()),
    }
}

// Bad: the step is applied to a `usize`, so the path is written under
// `usize`.
fn unsigned(counts: Vec<usize>) -> Vec<usize> {
    counts.into_iter().map(|count| count.to_string().len()).collect()
}

// Good: one adapter per step. Neither step takes what it is applied to by
// value, so each keeps a closure rather than a path.
fn split_unsigned(counts: Vec<usize>) -> Vec<usize> {
    counts
        .into_iter()
        .map(|count| count.to_string())
        .map(|text| text.len())
        .collect()
}

// Bad: the step is applied to an `i32`, whose path is written under the
// signed name rather than the unsigned one.
fn signed(offsets: Vec<i32>) -> Vec<String> {
    offsets.into_iter().map(|offset| offset.abs().to_string()).collect()
}

// Good: one adapter per step, `abs` under the path it takes by value and
// `to_string` keeping a closure because it borrows.
fn split_signed(offsets: Vec<i32>) -> Vec<String> {
    offsets
        .into_iter()
        .map(i32::abs)
        .map(|offset| offset.to_string())
        .collect()
}

// Bad: the step is applied to an `f64`.
fn floating(sizes: Vec<f64>) -> Vec<String> {
    sizes.into_iter().map(|size| size.abs().to_string()).collect()
}

// Good: one adapter per step.
fn split_floating(sizes: Vec<f64>) -> Vec<String> {
    sizes.into_iter().map(f64::abs).map(|size| size.to_string()).collect()
}

// Bad: the step is applied to a `bool`.
fn flags(flags: Vec<bool>) -> Vec<usize> {
    flags.into_iter().map(|flag| flag.to_string().len()).collect()
}

// Good: one adapter per step.
fn split_flags(flags: Vec<bool>) -> Vec<usize> {
    flags
        .into_iter()
        .map(|flag| flag.to_string())
        .map(|text| text.len())
        .collect()
}

// Bad: the step is applied to a `char`.
fn letters(letters: Vec<char>) -> Vec<usize> {
    letters.into_iter().map(|letter| letter.to_string().len()).collect()
}

// Good: one adapter per step.
fn split_letters(letters: Vec<char>) -> Vec<usize> {
    letters
        .into_iter()
        .map(|letter| letter.to_string())
        .map(|text| text.len())
        .collect()
}

// Bad: the first step is a call whose argument reaches the parameter through
// a `Deref` impl, `&PathBuf` to `&Path`. Naming the function would hand it a
// `&PathBuf`, which it does not take, so the step keeps a closure where a
// step handed exactly what it takes would not.
fn coerced(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.iter().filter_map(|file| canonical(file).ok()).collect()
}

// Good: one adapter per step, the coercing call keeping its closure and
// `ok` written as the path it can be.
fn split_coerced(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|file| canonical(file))
        .filter_map(Result::ok)
        .collect()
}

/// A type this file names only through its module, so its own name does not
/// resolve at the top level.
mod held {
    pub struct Wrapper(pub usize);

    impl Wrapper {
        pub fn value(self) -> usize {
            self.0
        }

        pub fn doubled(self) -> Wrapper {
            Wrapper(self.0 * 2)
        }

        pub fn counted(self) -> Counter {
            Counter(self.0)
        }
    }

    pub struct Counter(pub usize);

    impl Counter {
        pub fn count(self) -> usize {
            self.0
        }
    }
}

// Bad: the first step is a method on a type the file does not import, so the
// path form names something that does not resolve here. The rewrite is still
// the advice, offered with the `use` it wants.
fn unimported(items: Vec<held::Wrapper>) -> Vec<String> {
    items
        .into_iter()
        .map(|wrapper| wrapper.value().to_string())
        .collect()
}

// Good: one adapter per step, with the import the paths need.
fn split_unimported(items: Vec<held::Wrapper>) -> Vec<String> {
    use held::Wrapper;

    items
        .into_iter()
        .map(Wrapper::value)
        .map(|value| value.to_string())
        .collect()
}

// Bad: both steps are methods this file could name as paths, so the import is
// the only thing between the rewrite and compiling. That is what makes it
// advice rather than a fix the tooling applies on its own.
fn two_unimported_steps(items: Vec<held::Wrapper>) -> Vec<usize> {
    items
        .into_iter()
        .map(|wrapper| wrapper.doubled().value())
        .collect()
}

// Good: one adapter per step, both written as paths, with the import.
fn split_two_unimported_steps(items: Vec<held::Wrapper>) -> Vec<usize> {
    use held::Wrapper;

    items
        .into_iter()
        .map(Wrapper::doubled)
        .map(Wrapper::value)
        .collect()
}

// Bad: the two steps are applied to two types, neither of them imported, so
// the rewrite names two paths and wants a `use` for each.
fn two_unimported_types(items: Vec<held::Wrapper>) -> Vec<usize> {
    items.into_iter().map(|wrapper| wrapper.counted().count()).collect()
}

// Good: one adapter per step, with an import apiece.
fn split_two_unimported_types(items: Vec<held::Wrapper>) -> Vec<usize> {
    use held::{Counter, Wrapper};

    items
        .into_iter()
        .map(Wrapper::counted)
        .map(Counter::count)
        .collect()
}

/// The import a path form needs may sit in the module holding the chain
/// rather than at the crate root, which is the other place the rewrite looks.
mod nearby {
    use super::held::Wrapper;

    // Bad: the paths resolve here, this module importing the type its own
    // chain is over, so the rewrite needs no import of its own.
    fn imported(items: Vec<Wrapper>) -> Vec<usize> {
        items.into_iter().map(|wrapper| wrapper.doubled().value()).collect()
    }

    // Good: one adapter per step, both written as paths.
    fn split_imported(items: Vec<Wrapper>) -> Vec<usize> {
        items.into_iter().map(Wrapper::doubled).map(Wrapper::value).collect()
    }
}

fn main() {}
