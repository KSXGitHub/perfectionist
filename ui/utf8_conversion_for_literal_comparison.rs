// edition:2024
//
// Which conversions this rule reads as meeting only string literals,
// which comparison each is asked to become, and which shapes it leaves
// alone.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::ffi::{CStr, CString, OsStr, OsString};
use std::path::{Path, PathBuf};
use std::str;

// Bad: an equality over an OS string, which can keep its literal.
fn equality_over_os_strings(value: &OsStr, owned: OsString) {
    if value.to_str() == Some("true") {}
    if value.to_str() != Some("true") {}
    // The literal may sit on either side.
    if Some("true") == value.to_str() {}
    if owned.into_string().unwrap() == "true" {}
}

// Good: `equality_over_os_strings` as the rule asks for it. An `OsStr`
// carries `PartialEq<str>`, so every literal keeps its text.
fn equality_over_os_strings_rewritten(value: &OsStr, owned: OsString) {
    if value == "true" {}
    if value != "true" {}
    if "true" == value {}
    if owned == "true" {}
}

// Bad: an `expect` rather than an `unwrap` between the conversion and
// the comparison. Either hands back the text the conversion produced.
fn an_expect_between(path: &Path) -> bool {
    path.to_str().expect("not UTF-8") == "main.rs"
}

// Good: `an_expect_between` as the rule asks for it. The rewrite drops
// the `expect`, so input the conversion would have rejected answers
// `false` where it used to panic.
fn an_expect_between_rewritten(path: &Path) -> bool {
    path.as_os_str() == "main.rs"
}

// Bad: an equality over a path, which reaches its `OsStr` through
// `as_os_str`.
fn equality_over_paths(path: &Path, owned: PathBuf) {
    if path.to_str() == Some("main.rs") {}
    if owned.to_str() == Some("main.rs") {}
}

// Good: `equality_over_paths` as the rule asks for it. Comparing the
// `Path` itself is not the counterpart: `Path` compares by normalised
// components, so it would answer differently.
fn equality_over_paths_rewritten(path: &Path, owned: PathBuf) {
    if path.as_os_str() == "main.rs" {}
    if owned.as_os_str() == "main.rs" {}
}

// Bad: a lossy conversion, which the rule reads beside the ones that
// report ill-formed input instead of replacing it.
fn a_lossy_conversion(path: &Path) -> bool {
    path.to_string_lossy() == "main.rs"
}

// Good: `a_lossy_conversion` as the rule asks for it. The replacement
// the conversion would make has no pre-image among the inputs that
// render as this literal, so the two answer alike.
fn a_lossy_conversion_rewritten(path: &Path) -> bool {
    path.as_os_str() == "main.rs"
}

// Bad: an equality over a byte buffer, which is already its own native
// form.
fn equality_over_bytes(bytes: &[u8], owned: Vec<u8>, boxed: Box<[u8]>) {
    if str::from_utf8(bytes) == Ok("yes") {}
    if String::from_utf8_lossy(bytes) == "yes" {}
    if String::from_utf8(owned).unwrap() == "yes" {}
    if str::from_utf8(&boxed).unwrap() == "yes" {}
}

// Good: `equality_over_bytes` as the rule asks for it. A `Vec<u8>`
// meets the byte-string literal as it stands, where a `Box<[u8]>` has
// to be sliced to meet one.
fn equality_over_bytes_rewritten(bytes: &[u8], owned: Vec<u8>, boxed: Box<[u8]>) {
    if bytes == b"yes" {}
    if owned == b"yes" {}
    if &boxed[..] == b"yes" {}
}

// Bad: a dereferenced source, whose brackets the rewrite has to keep,
// a method appended to `*path` binding tighter than the `*` does.
fn a_dereferenced_source(path: &&Path) -> bool {
    (*path).to_str() == Some("main.rs")
}

// Good: `a_dereferenced_source` as the rule asks for it, the brackets
// still holding `as_os_str` to the path rather than to its result.
fn a_dereferenced_source_rewritten(path: &&Path) -> bool {
    (*path).as_os_str() == "main.rs"
}

// Bad: an equality over a fixed-size buffer, which meets a
// byte-string literal only at its own length.
fn equality_over_a_fixed_buffer(buffer: [u8; 3]) -> bool {
    str::from_utf8(&buffer) == Ok("yes")
}

// Good: `equality_over_a_fixed_buffer` as the rule asks for it, sliced
// so that the literal's length no longer has to agree.
fn equality_over_a_fixed_buffer_rewritten(buffer: [u8; 3]) -> bool {
    &buffer[..] == b"yes"
}

// Bad: an equality over a C string, whose byte view drops the
// terminator the literal never mentions.
fn equality_over_c_strings(value: &CStr, owned: CString) {
    if value.to_str() == Ok("yes") {}
    if owned.to_str().unwrap() == "yes" {}
}

// Good: `equality_over_c_strings` as the rule asks for it. Neither
// view is a `_with_nul` one, which would retain a terminator no
// literal can carry.
fn equality_over_c_strings_rewritten(value: &CStr, owned: CString) {
    if value.to_bytes() == b"yes" {}
    if owned.as_bytes() == b"yes" {}
}

// Bad: a `match` deciding which of three words a value is, where every
// arm carries a `Some` that no arm asked for.
fn match_over_an_os_string(value: &OsStr) -> Result<bool, ()> {
    match value.to_str() {
        Some("true") => Ok(true),
        Some("false") | Some("") => Ok(false),
        _ => Err(()),
    }
}

// Good: `match_over_an_os_string` as the rule asks for it, with the
// arms spelled as the words they decide between.
fn match_over_an_os_string_rewritten(value: &OsStr) -> Result<bool, ()> {
    match value.as_encoded_bytes() {
        b"true" => Ok(true),
        b"false" | b"" => Ok(false),
        _ => Err(()),
    }
}

// Bad: a `matches!` expands to a `match`, so the rule reads its arms
// as literal patterns.
fn matches_over_a_path(path: &Path) -> bool {
    matches!(path.to_str(), Some("lib.rs" | "main.rs" | "mod.rs"))
}

// Good: `matches_over_a_path` as the rule asks for it. A string
// literal in pattern position matches a `str` and nothing else, so
// this one compares bytes.
fn matches_over_a_path_rewritten(path: &Path) -> bool {
    matches!(
        path.as_os_str().as_encoded_bytes(),
        b"lib.rs" | b"main.rs" | b"mod.rs",
    )
}

// Bad: the conversion reached through an `Option` adapter. The
// comparison already carries the `Some` the adapter answers with.
fn through_an_option_adapter(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("main.rs")
}

// Good: `through_an_option_adapter` as the rule asks for it, keeping
// the `Some` and reaching bytes in place of the conversion the closure
// performed. An `Option`'s equality is between two of the same type,
// which is what the literal is sliced for.
fn through_an_option_adapter_rewritten(path: &Path) -> bool {
    path.file_name().map(|value| value.as_encoded_bytes()) == Some(b"main.rs".as_slice())
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

// Bad: a `match` whose other arm names the conversion's own failure
// alongside the text it could not place.
fn the_other_arm_names_the_failure(value: &OsStr) -> u8 {
    match value.to_str() {
        Some("true") => 1,
        Some(_) | None => 0,
    }
}

// Good: `the_other_arm_names_the_failure` as the rule asks for it. The
// comparison left behind cannot fail, so the arm that named the
// failure becomes a wildcard.
fn the_other_arm_names_the_failure_rewritten(value: &OsStr) -> u8 {
    match value.as_encoded_bytes() {
        b"true" => 1,
        _ => 0,
    }
}

// Bad: a fallible conversion whose other arm names the error it
// reports.
fn the_other_arm_names_an_error(value: &CStr) -> u8 {
    match value.to_str() {
        Ok("yes") => 1,
        Ok(_) | Err(_) => 0,
    }
}

// Good: `the_other_arm_names_an_error` as the rule asks for it, the
// error arm becoming a wildcard for want of anything left to name.
fn the_other_arm_names_an_error_rewritten(value: &CStr) -> u8 {
    match value.to_bytes() {
        b"yes" => 1,
        _ => 0,
    }
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

// Bad: a closure parameter, carrying a second layer of reference that
// the comparison does not reach through.
fn through_a_second_reference(names: &[&OsStr]) -> bool {
    names.iter().any(|name| name.to_str() == Some("main.rs"))
}

// Good: `through_a_second_reference` as the rule asks for it, with the
// dereference that leaves the one layer the comparison reaches
// through.
fn through_a_second_reference_rewritten(names: &[&OsStr]) -> bool {
    names.iter().any(|name| *name == "main.rs")
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

// Not flagged: the arm binds the decoded text, so the conversion is
// what lets the arm use it.
fn an_arm_binds_the_text(path: &Path) -> Option<String> {
    match path.to_str() {
        Some("-") => None,
        Some(other) => Some(other.to_owned()),
        None => None,
    }
}

// Bad: a guard on a literal arm. The guard weighs something other than
// the text, and a pattern naming only literals leaves it nothing of the
// text to read, so the rewrite carries the guard across untouched.
fn an_arm_carries_a_guard(value: &OsStr, strict: bool) -> u8 {
    match value.to_str() {
        Some("true") if strict => 1,
        _ => 0,
    }
}

// Good: `an_arm_carries_a_guard` as the rule asks for it, the guard
// standing where it was written.
fn an_arm_carries_a_guard_rewritten(value: &OsStr, strict: bool) -> u8 {
    match value.as_encoded_bytes() {
        b"true" if strict => 1,
        _ => 0,
    }
}

// Not flagged: the arm binds the decoded text for its guard to read, so
// the conversion is what lets the guard ask what it asks.
fn a_guard_reads_the_binding(path: &Path) -> bool {
    match path.to_str() {
        Some(name) if name.starts_with("main") => true,
        _ => false,
    }
}

// Not flagged: the non-UTF-8 case has an outcome of its own, which a
// byte comparison cannot keep apart from unlisted text.
fn the_non_utf8_case_is_distinguished(path: &Path) -> u8 {
    match path.to_str() {
        Some("-") => 0,
        Some(_) => 1,
        None => 2,
    }
}

// Not flagged: an ordering comparison rather than an equality. An
// `Option` orders its `None` below every `Some`, which no comparison
// of the value's own form reproduces.
fn an_ordering_comparison(value: &OsStr) -> bool {
    value.to_str() < Some("true")
}

// Not flagged: the arm names a constant rather than a literal. The
// rewrite spells its literals as bytes, which a constant's name does
// not carry.
fn an_arm_names_a_constant(path: &Path) -> u8 {
    match path.to_str() {
        Some(WANTED) => 1,
        _ => 0,
    }
}

// Not flagged: the error arm binds what the conversion reported, so the
// `match` uses the conversion's answer rather than discarding it.
fn an_error_arm_binds_the_error(value: &CStr) -> usize {
    match value.to_str() {
        Ok("yes") => 0,
        Ok(_) => 1,
        Err(error) => error.valid_up_to(),
    }
}

// Not flagged: the comparand is built by a call the rule cannot look
// inside, so there is no literal behind it.
fn the_comparand_is_a_supplied_call(decoder: &Decoder, value: &OsStr) -> bool {
    value.to_str() == (decoder.convert)(value)
}

// Not flagged: a conversion consumed through an adapter other than
// `unwrap` or `expect`, which the rule does not read through.
fn the_conversion_is_defaulted(value: &OsStr) -> bool {
    value.to_str().unwrap_or("") == "true"
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

// Not flagged: the comparand is not a literal, so there is nothing to
// compare the native form against.
fn the_comparand_is_not_a_literal(path: &Path, wanted: &str) -> bool {
    path.to_str() == Some(wanted)
}

// Not flagged: a lossy conversion against a literal holding U+FFFD. A
// lone `0xFF` renders as U+FFFD and compares equal, where its encoded
// bytes do not.
fn a_lossy_conversion_against_a_replacement_character(path: &Path) -> bool {
    path.to_string_lossy() == "�"
}

// Not flagged: a lossy conversion taken as a function, where the
// literal holds U+FFFD. The replacement character has pre-images among
// the ill-formed inputs, which the byte comparison does not share.
fn a_lossy_function_against_a_replacement_character(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes) == "�"
}

// Not flagged: the receiver is already a `str`, which has no non-UTF-8
// native form to compare instead.
fn the_receiver_is_already_text(text: &str) -> bool {
    text.to_lowercase() == "true"
}

// Not flagged: the closure converts something other than its own
// parameter, so the comparison is not asking about the adapter's
// receiver.
fn the_closure_converts_elsewhere(path: &Path, other: &OsStr) -> bool {
    path.file_name().and_then(|_| other.to_str()) == Some("main.rs")
}

// Not flagged: an alternation naming both a literal and everything
// else. The arm reaches every input, so the rewrite has nothing to put
// in place of the literal inside it.
fn an_alternation_mixes_a_literal_and_a_wildcard(path: &Path) -> u8 {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("lib.rs") => 1,
        Some("main.rs" | _) | None => 2,
    }
}

// Not flagged: no arm names a literal, so the `match` is not asking
// which word the text is.
fn no_arm_names_a_literal(value: &OsStr) -> u8 {
    match value.to_str() {
        _ => 1,
    }
}

macro_rules! word {
    () => {
        "true"
    };
}

// Not flagged: the literal itself comes from a macro, so a rewrite
// would name whatever that expands to today.
fn the_literal_comes_from_a_macro(value: &OsStr) -> bool {
    value.to_str() == Some(word!())
}

// Not flagged: an arm's literal comes from a macro. The rewrite
// spells out each literal it reads, so it would freeze that macro's
// expansion into the source.
fn an_arm_literal_comes_from_a_macro(value: &OsStr) -> u8 {
    match value.to_str() {
        Some(word!()) => 1,
        _ => 0,
    }
}

macro_rules! decoded {
    ($value:expr) => {
        $value.to_str()
    };
}

// Not flagged: the conversion itself comes from a macro, whose body is
// where the text a rewrite would name may live.
fn the_conversion_comes_from_a_macro(value: &OsStr) -> bool {
    decoded!(value).unwrap() == "true"
}

macro_rules! wrapped {
    ($literal:literal) => {
        Some($literal)
    };
}

// Not flagged: the comparand's wrapper comes from a macro, so a
// rewrite would edit the macro's own text through this one call site.
fn the_comparand_comes_from_a_macro(value: &OsStr) -> bool {
    value.to_str() == wrapped!("true")
}

const WANTED: &str = "main.rs";

struct Decoder {
    convert: fn(&OsStr) -> Option<&str>,
}

trait DecodeText {
    fn to_str(&self) -> Option<&str>;
}

impl DecodeText for Vec<u16> {
    fn to_str(&self) -> Option<&str> {
        None
    }
}

impl DecodeText for [u16] {
    fn to_str(&self) -> Option<&str> {
        None
    }
}

impl DecodeText for u16 {
    fn to_str(&self) -> Option<&str> {
        None
    }
}

impl DecodeText for Box<str> {
    fn to_str(&self) -> Option<&str> {
        None
    }
}

// Not flagged: a `Vec` over something other than bytes is not a byte
// buffer, whatever it names its decoding method.
fn not_a_byte_buffer(values: Vec<u16>) -> bool {
    values.to_str() == Some("yes")
}

// Not flagged: a value whose type is no kind of buffer at all.
fn not_a_buffer(value: u16) -> bool {
    value.to_str() == Some("yes")
}

// Not flagged: a `Box` over text rather than over a slice of bytes.
fn not_a_boxed_buffer(value: Box<str>) -> bool {
    value.to_str() == Some("yes")
}

// Not flagged: a slice over something other than bytes. The rule
// reads the element type rather than trusting the shape.
fn not_a_byte_slice(values: &[u16]) -> bool {
    values.to_str() == Some("yes")
}

mod elsewhere {
    pub struct Path(pub String);

    impl Path {
        pub fn to_str(&self) -> Option<&str> {
            Some(&self.0)
        }
    }
}

// Not flagged: a `Path` defined outside the standard library, which
// shares the name and holds UTF-8 already.
fn a_path_defined_elsewhere(path: &elsewhere::Path) -> bool {
    path.to_str() == Some("main.rs")
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
