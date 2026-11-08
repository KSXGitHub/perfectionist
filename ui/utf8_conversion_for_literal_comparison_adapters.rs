// edition:2024
//
// The conversion reached inside an `Option` adapter's closure: what the
// rewrite keeps of the comparison around it, and which receivers and
// closures leave the adapter unread.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::ffi::OsStr;
use std::path::Path;
use std::str;

// Bad: the conversion reached through an `Option` adapter. The
// comparison already carries the `Some` the adapter answers with.
fn through_an_option_adapter(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("main.rs")
}

// Good: `through_an_option_adapter` as the rule asks for it, keeping
// the `Some` and reaching bytes in place of the conversion the closure
// performed. The literal needs no slice: the argument of `Some` is read
// after the operand it meets has named the type, so the array behind
// the literal reaches an `Option` of a slice as it stands.
fn through_an_option_adapter_rewritten(path: &Path) -> bool {
    path.file_name().map(|value| value.as_encoded_bytes()) == Some(b"main.rs")
}

// Bad: an adapter whose comparison carries the literal on the left,
// which is the one order where the rewrite has to spell the literal as
// a slice: the comparand is typed before the operand it meets, so
// nothing has named the type the array would reach for.
fn an_adapter_with_the_literal_on_the_left(path: &Path) -> bool {
    Some("main.rs") == path.file_name().and_then(|name| name.to_str())
}

// Good: `an_adapter_with_the_literal_on_the_left` as the rule asks for
// it, the literal sliced so that it meets an `Option` of a slice.
fn an_adapter_with_the_literal_on_the_left_rewritten(path: &Path) -> bool {
    Some(b"main.rs".as_slice()) == path.file_name().map(|value| value.as_encoded_bytes())
}

// Bad: an `Option` adapter whose consumer is a `matches!` rather than
// an equality.
fn through_an_option_adapter_in_a_match(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("lib.rs" | "main.rs"),
    )
}

// Good: `through_an_option_adapter_in_a_match` as the rule asks for
// it, the adapter reaching bytes so the arms can be literals.
fn through_an_option_adapter_in_a_match_rewritten(path: &Path) -> bool {
    matches!(
        path.file_name().map(|value| value.as_encoded_bytes()),
        Some(b"lib.rs" | b"main.rs"),
    )
}

// Bad: an `Option` adapter whose other arm names the failure. The
// adapter's `Option` survives the rewrite, so that arm survives it
// too.
fn an_adapter_keeps_the_failure_arm(path: &Path) -> u8 {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("main.rs") => 1,
        Some(_) | None => 0,
    }
}

// Good: `an_adapter_keeps_the_failure_arm` as the rule asks for it,
// with the arm that named the failure left as it was written.
fn an_adapter_keeps_the_failure_arm_rewritten(path: &Path) -> u8 {
    match path.file_name().map(|value| value.as_encoded_bytes()) {
        Some(b"main.rs") => 1,
        Some(_) | None => 0,
    }
}

// Bad: an `Option::map` adapter, whose closure answers an `Option` of
// its own, so the pattern names two `Some`s.
fn through_a_map_adapter(path: &Path) -> bool {
    matches!(
        path.file_name().map(|name| name.to_str()),
        Some(Some("main.rs")),
    )
}

// Good: `through_a_map_adapter` as the rule asks for it, the adapter
// answering the bytes so that one `Some` is left.
fn through_a_map_adapter_rewritten(path: &Path) -> bool {
    matches!(
        path.file_name().map(|value| value.as_encoded_bytes()),
        Some(b"main.rs"),
    )
}

// Bad: an adapter over a byte slice, which holds the bytes the
// comparison wants, so the rewrite drops the closure outright.
fn through_an_adapter_over_bytes(chunk: Option<&[u8]>) -> bool {
    matches!(chunk.map(|bytes| str::from_utf8(bytes)), Some(Ok("yes")))
}

// Good: `through_an_adapter_over_bytes` as the rule asks for it, the
// adapter gone because its receiver already answers in bytes.
fn through_an_adapter_over_bytes_rewritten(chunk: Option<&[u8]>) -> bool {
    matches!(chunk, Some(b"yes"))
}

// Not flagged: the adapter is an array's `map` rather than an
// `Option`'s, so the comparison carries no `Some` for the rewrite to
// keep.
fn the_adapter_is_over_an_array(value: &OsStr) -> bool {
    [value].map(|name| name.to_str()) == [Some("true")]
}

// Not flagged: the adapter is handed a function rather than a closure,
// so there is no closure body to read a conversion out of.
fn the_adapter_takes_a_function(path: &Path) -> bool {
    matches!(path.file_name().map(OsStr::to_str), Some(Some("main.rs")))
}

// Not flagged: the closure converts an expression built from its
// parameter rather than the parameter itself, which the rule reads as
// asking about something other than the adapter's receiver.
fn the_closure_converts_a_derived_value(names: Option<&&OsStr>) -> bool {
    names.and_then(|name| (*name).to_str()) == Some("main.rs")
}

// Not flagged: the closure converts something other than its own
// parameter, so the comparison is not asking about the adapter's
// receiver.
fn the_closure_converts_elsewhere(path: &Path, other: &OsStr) -> bool {
    path.file_name().and_then(|_| other.to_str()) == Some("main.rs")
}

struct Unwrapper<'a>(&'a OsStr);

impl<'a> Unwrapper<'a> {
    fn map<Mapped>(self, take: impl FnOnce(&'a OsStr) -> Mapped) -> Mapped {
        take(self.0)
    }
}

// Not flagged: the adapter's receiver is not an `Option`, so the
// `Some` the comparison carries is not the adapter's answer.
fn the_adapter_receiver_is_not_an_option(wrapper: Unwrapper<'_>) -> bool {
    wrapper.map(|name| name.to_str()) == Some("main.rs")
}

fn main() {}
