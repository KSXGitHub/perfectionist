// edition:2024
//
// What makes the lifted part compile. A lifted step runs in an adapter of
// its own, so a result borrowing the closure's own item dies with it, a
// capture only one closure could hold cannot go to two, a parallel adapter
// wants what the step produces to be `Send`, and a receiver the folded form
// left in place has to survive being moved.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn record(length: usize) {}

fn maybe(text: &str) -> Option<usize> {
    text.parse().ok()
}

fn bump(seen: &mut Vec<usize>) -> usize {
    seen.push(0);
    seen.len()
}



// Not flagged: `any` takes `&mut self`, so it leaves the receiver
// positioned and usable, and the leading `map` would move it. A receiver
// named again is `E0382` once split.
fn receiver_used_after(mut lines: std::vec::IntoIter<&'static str>) -> (bool, usize) {
    let empty = lines.any(|line| line.trim().is_empty());
    (empty, lines.count())
}



// Not flagged: an owned item, whose borrow would not outlive the `map`
// the step would move into.
fn owned_item(headers: std::vec::IntoIter<String>) -> Vec<String> {
    headers
        .map(|header| header.trim().to_ascii_lowercase())
        .collect()
}



// Bad: a fallible call applied to a step, which is two steps and no
// `Option` work, so the chain is what splits.
fn parse_the_trimmed(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| maybe(line.trim())).collect()
}



// Good: the step lifted, leaving the adapter the fallible call.
fn split_parse_the_trimmed(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(str::trim).filter_map(maybe).collect()
}



// Bad: the same over items that are `&String`, where what the step borrows
// sits behind the reference and so outlives the closure.
fn parse_the_trimmed_borrowed(lines: std::slice::Iter<'static, String>) -> Vec<usize> {
    lines.filter_map(|line| maybe(line.trim())).collect()
}



// Good: the step lifted. `str::trim` will not do here, a function item
// taking no deref coercion, so the closure stays and holds one step.
fn split_parse_the_trimmed_borrowed(lines: std::slice::Iter<'static, String>) -> Vec<usize> {
    lines.map(|line| line.trim()).filter_map(maybe).collect()
}



// Not flagged: items that own their text, where the lifted step would
// return a borrow of the closure's own parameter and be `E0515`.
fn parse_the_trimmed_owned(lines: std::vec::IntoIter<String>) -> Vec<usize> {
    lines.filter_map(|line| maybe(line.trim())).collect()
}


// Bad: a `&mut` item is a reference the closure was handed, so a step
// borrowing through it lifts. It is not `Copy`, which is why liftability
// asks whether the step borrows through a reference rather than whether
// the item can be copied.
fn mutable_reference_item(rows: std::slice::IterMut<'static, String>) -> Vec<usize> {
    rows.map(|row| row.as_str().len()).collect()
}


// Good: the step lifted, the borrow it hands on being of what the
// reference points at.
fn split_mutable_reference_item(rows: std::slice::IterMut<'static, String>) -> Vec<usize> {
    rows.map(|row| row.as_str()).map(str::len).collect()
}


// Not flagged: a `Copy` item that is not a reference. Copying four bytes
// is cheap and beside the point: the step borrows the closure's own
// parameter, which the lifted `map` ends before the borrow is used, so the
// split is `E0515`.
fn copy_item(rows: std::vec::IntoIter<[u8; 4]>) -> Vec<usize> {
    rows.map(|row| row.as_slice().len()).collect()
}



// Not flagged: the second step is applied to an owned `String`, which
// dies at the end of the `map` it would lift into, however the item
// arrived.
fn owned_intermediate(lines: std::vec::IntoIter<&'static str>) -> Vec<String> {
    lines
        .map(|line| line.to_lowercase().trim().to_string())
        .collect()
}



// Not flagged: a step mutably borrowing a capture the closure holds
// too, which two closures could not both do.
fn mutable_capture_in_a_step(lines: std::vec::IntoIter<&'static str>) -> usize {
    let mut seen = Vec::new();
    lines.fold(0, |total, line| {
        total + line.trim().len().min(bump(&mut seen))
    })
}



// Bad: a shared capture is one two closures may both hold, so a step
// reaching it still splits.
fn shared_capture_in_a_step(limit: usize, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| total + line.trim().len().min(limit))
}


// Good: the steps lifted, the capture going with the step that reached it.
fn split_shared_capture_in_a_step(
    limit: usize,
    lines: std::vec::IntoIter<&'static str>,
) -> usize {
    lines
        .map(str::trim)
        .map(str::len)
        .map(|length| length.min(limit))
        .fold(0, |total, length| total + length)
}



// Bad: a step whose result cannot cross a thread, which only a parallel
// adapter asks of it.
fn not_sendable_sequentially(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| std::rc::Rc::new(line).len())
        .collect()
}


// Good: the step lifted, which a sequential adapter asks nothing of. One
// `Rc` per item either way.
fn split_not_sendable_sequentially(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(std::rc::Rc::new)
        .map(|held| held.len())
        .collect()
}



// Bad: an argument that is a parameter of the method's own lends the
// result nothing, however the call instantiates it. `trim_start_matches`
// takes a `P: Pattern`, so the `&str` passed here carries a region the
// erased types cannot tell from the result's, where the declared signature
// can.
fn trimmed_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| line.trim_start_matches("# ").len())
        .collect()
}


// Good: the step lifted, its pattern argument going with it.
fn split_trimmed_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| line.trim_start_matches("# "))
        .map(str::len)
        .collect()
}



struct Lender(&'static str);

impl Lender {
    fn pick<'a>(&'a self, fallback: &'a str) -> &'a str {
        match self.0.is_empty() {
            true => fallback,
            false => self.0,
        }
    }
}



// Not flagged: this signature gives the result the argument's own region,
// so what the result borrows may be the temporary the closure made. That
// is the other half of what the declared signature answers.
fn region_from_an_argument(rows: std::slice::Iter<'static, Lender>) -> Vec<usize> {
    rows.map(|row| row.pick(&String::from("m")).len()).collect()
}



// Not flagged: the step's result borrows the shorter of two lifetimes,
// and the shorter one is a temporary this closure made, so the receiver
// being a reference does not answer for it.
fn argument_lends_the_result(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| line.max(&String::from("m")[..]).len())
        .collect()
}



// Not flagged: a `ref` item types as a reference to the closure's own
// parameter slot, so a step borrowing through it borrows what dies with
// the closure.
fn ref_item(headers: std::vec::IntoIter<String>) -> Vec<usize> {
    headers.map(|ref header| header.trim().len()).collect()
}



// Not flagged: a callee that is not a path can hand back an `Fn` holding
// a borrow, which the step's result then carries.
fn pick<'chosen>(prefix: &'chosen str) -> impl Fn(&str) -> &'chosen str {
    move |_| prefix
}



fn callee_lends_the_result(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| pick(&String::from("m"))(line).len())
        .collect()
}



// Not flagged: a `move` closure writing to a `Copy` capture would give
// each half its own copy, which compiles and answers differently.
fn move_closure_writes_its_capture(lines: std::vec::IntoIter<&'static str>) -> usize {
    let mut seen = 0usize;
    lines.fold(0, move |total, line| {
        seen += 1;
        total + line.trim().len().min(seen)
    })
}



// Bad: a `move` closure holds its own copy of a `Copy` capture, so two of
// them may both read it.
fn move_closure_copies_its_capture(
    limit: usize,
    lines: std::vec::IntoIter<&'static str>,
) -> usize {
    lines.fold(0, move |total, line| {
        total + line.trim().len().min(limit)
    })
}


// Good: the steps lifted, each closure holding its own copy of the
// capture, which is what lets both of them have it.
fn split_move_closure_copies_its_capture(
    limit: usize,
    lines: std::vec::IntoIter<&'static str>,
) -> usize {
    lines
        .map(str::trim)
        .map(str::len)
        .map(move |length| length.min(limit))
        .fold(0, |total, length| total + length)
}

fn main() {}
