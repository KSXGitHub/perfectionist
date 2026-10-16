// aux-build:pipe_trait_stub.rs
// edition:2024
//
// `pipe-trait`'s piping methods, which the rule identifies by path because
// the trait carries no diagnostic item. They are the shape where no step
// stays behind: every one becomes a `pipe` of its own, and the head keeps
// whichever method the receiver was handed through, because that is what
// decides whether the closure gets the value, a shared borrow, a mutable
// borrow, or a conversion of one.
//
// The crate is not in the driver's sysroot, so a stub stands in for it,
// carrying only the methods these cases call.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate pipe_trait;

use pipe_trait::Pipe;

fn double(value: usize) -> usize {
    value * 2
}

fn render(value: usize) -> String {
    value.to_string()
}

// Bad: a piping method, where every step becomes one and none stays.
fn piped(value: usize) -> String {
    value.pipe(|number| render(double(number)))
}

// Good: one `pipe` per step.
fn split_piped(value: usize) -> String {
    value.pipe(double).pipe(render)
}

// Not flagged: `pipe` moves the receiver into the closure, so a step
// returning a borrow of it dies with the closure.
fn piped_borrowing(value: String) -> usize {
    value.pipe(|owned| owned.trim().len())
}

// Bad: `pipe_ref` hands the closure a shared borrow, so a step borrowing
// through it outlives the closure and lifts.
fn piped_by_reference(value: String) -> usize {
    value.pipe_ref(|text| text.trim().len())
}

// Good: the head keeps the method, which is what keeps the receiver
// borrowed rather than moved.
fn split_piped_by_reference(value: String) -> usize {
    value.pipe_ref(|text| text.trim()).pipe(str::len)
}

// Bad: three steps rather than two, so the lifting runs past one step.
fn piped_by_reference_thrice(value: String) -> usize {
    value.pipe_ref(|text| text.trim().to_uppercase().len())
}

// Good: the head keeps the method, and both steps above it take the value a
// `pipe` hands them.
fn split_piped_by_reference_thrice(value: String) -> usize {
    value
        .pipe_ref(|text| text.trim())
        .pipe(str::to_uppercase)
        .pipe(|upper| upper.len())
}

// Bad: `pipe_mut` hands the closure a mutable borrow, which a step may
// borrow through just as it may a shared one.
fn piped_mutably(mut value: Vec<usize>) -> usize {
    value.pipe_mut(|all| all.as_mut_slice().len())
}

// Good: the head keeps `pipe_mut`, which is what keeps the borrow mutable.
fn split_piped_mutably(mut value: Vec<usize>) -> usize {
    value.pipe_mut(|all| all.as_mut_slice()).pipe(|all| all.len())
}

// Bad: `pipe_as_ref` converts through `AsRef` before handing the borrow
// over, and the step borrows through what the conversion produced.
fn piped_as_reference(value: String) -> usize {
    value.pipe_as_ref(|text: &str| text.trim().len())
}

// Good: the head keeps `pipe_as_ref`, so the conversion still happens once.
fn split_piped_as_reference(value: String) -> usize {
    value.pipe_as_ref(|text: &str| text.trim()).pipe(str::len)
}

// Bad: `pipe_as_mut` converts through `AsMut`, the mutable counterpart of
// `AsRef`.
fn piped_as_mutable(mut value: String) -> usize {
    value.pipe_as_mut(|text: &mut str| text.trim().len())
}

// Good: the head keeps `pipe_as_mut`, so the conversion still happens once.
fn split_piped_as_mutable(mut value: String) -> usize {
    value.pipe_as_mut(|text: &mut str| text.trim()).pipe(str::len)
}

// Bad: `pipe_deref` reaches the borrow through `Deref` rather than a
// conversion the caller names.
fn piped_through_deref(value: String) -> usize {
    value.pipe_deref(|text: &str| text.trim().len())
}

// Good: the head keeps `pipe_deref`, so the deref still happens once.
fn split_piped_through_deref(value: String) -> usize {
    value.pipe_deref(|text: &str| text.trim()).pipe(str::len)
}

// Bad: `pipe_deref_mut` reaches a mutable borrow through `DerefMut`.
fn piped_through_deref_mut(mut value: String) -> usize {
    value.pipe_deref_mut(|text: &mut str| text.trim().len())
}

// Good: the head keeps `pipe_deref_mut`, so the deref still happens once.
fn split_piped_through_deref_mut(mut value: String) -> usize {
    value.pipe_deref_mut(|text: &mut str| text.trim()).pipe(str::len)
}

// Bad: `pipe_borrow` reaches the borrow through `Borrow`, which a type may
// implement for several targets.
fn piped_through_borrow(value: String) -> usize {
    value.pipe_borrow(|text: &str| text.trim().len())
}

// Good: the head keeps `pipe_borrow`, so the borrow is still of the target
// the annotation picked.
fn split_piped_through_borrow(value: String) -> usize {
    value.pipe_borrow(|text: &str| text.trim()).pipe(str::len)
}

// Bad: `pipe_borrow_mut` reaches a mutable borrow through `BorrowMut`.
fn piped_through_borrow_mut(mut value: String) -> usize {
    value.pipe_borrow_mut(|text: &mut str| text.trim().len())
}

// Good: the head keeps `pipe_borrow_mut`, so the borrow is still mutable
// and still of the target the annotation picked.
fn split_piped_through_borrow_mut(mut value: String) -> usize {
    value.pipe_borrow_mut(|text: &mut str| text.trim()).pipe(str::len)
}

// Bad: a receiver named again after the call, which the split borrows
// rather than moves, so there is nothing for the receiver-move gate to
// decline.
fn piped_by_reference_then_used(value: String) -> (usize, String) {
    let length = value.pipe_ref(|text| text.trim().len());
    (length, value)
}

// Good: the split borrows the receiver where the folded form did, so the
// name is still there afterwards.
fn split_piped_by_reference_then_used(value: String) -> (usize, String) {
    let length = value.pipe_ref(|text| text.trim()).pipe(str::len);
    (length, value)
}

fn main() {}
