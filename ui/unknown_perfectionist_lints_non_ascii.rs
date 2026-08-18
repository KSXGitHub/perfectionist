#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    mixed_script_confusables,
    uncommon_codepoints,
    perfectionist::allow_attributes_without_reason,
    perfectionist::lint_attribute_trailing_comment,
    perfectionist::allow_attributes,
    reason = "ui fixture",
)]

// Bad: a homoglyph typo of `unicode_ellipsis_in_comments` — the `o` of
// `comments` is a Cyrillic `о`. `mixed_script_confusables` is rustc's
// own crate-level complaint about that character and is allowed above
// so this fixture shows only what perfectionist adds.
#[allow(perfectionist::unicode_ellipsis_in_cоmments)]
fn homoglyph() {}

// Bad: the offending character is a combining mark. Reported raw it
// would ride the quote of the help message, so it is escaped. The
// help reads `'\u{951}'`; compiletest rewrites every backslash to a
// forward slash when it normalises expected output for Windows, hence
// the `/u{951}` in the stderr file.
#[allow(perfectionist::wildcard_imports॑)]
fn combining_mark() {}

fn main() {}
