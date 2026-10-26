//! Apply the unified diff a rustfix fixture stores to the fixture's
//! own text, which is how the fixed form it is held to is reached.
//!
//! What a `// run-rustfix` fixture is compared against is the whole
//! fixed source, which compiletest reads from a `.fixed` beside it.
//! Committing that file would commit a second copy of the fixture to
//! show the handful of lines a rewrite touches, so what is committed
//! is the diff and the `.fixed` is materialised from it at test time.
//!
//! The diff is the one `git diff --no-index` writes: two header
//! lines and then a hunk per region, with context either side.
//! Nothing here guesses at a hunk that does not fit. A context or
//! removed line that does not match the fixture means the fixture
//! moved under its diff, and saying so beats patching the wrong
//! region.

use core::str::Lines;

/// What a hunk header carries: where the hunk starts in the original,
/// 1-based, as `@@ -<start>,<count> +<start>,<count> @@` spells it.
/// Only the original's start is read; the rest is the arithmetic the
/// body already performs.
struct Hunk {
    start: usize,
}

/// The fixed form of `original`, with `diff` applied.
///
/// `what` names the diff in a failure, and reads as the subject of
/// "does not apply".
pub(super) fn apply(original: &str, diff: &str, what: &str) -> String {
    let mut lines = original.lines();
    let mut fixed: Vec<&str> = Vec::new();
    let mut taken = 0;
    for line in diff.lines() {
        if let Some(hunk) = take_hunk_header(line) {
            assert!(
                hunk.start > taken,
                "{what} does not apply: a hunk starts at line {} of the fixture, \
                 which an earlier hunk already covered",
                hunk.start,
            );
            while taken + 1 < hunk.start {
                fixed.push(next(&mut lines, what));
                taken += 1;
            }
            continue;
        }
        let Some((marker, text)) = split_marker(line) else {
            continue;
        };
        match marker {
            ' ' | '-' => {
                let found = next(&mut lines, what);
                taken += 1;
                assert_eq!(
                    found, text,
                    "{what} does not apply: line {taken} of the fixture reads \
                     differently from what the diff expects there",
                );
                if marker == ' ' {
                    fixed.push(found);
                }
            }
            '+' => fixed.push(text),
            _ => unreachable!("split_marker yields only a space, a minus or a plus"),
        }
    }
    for line in lines {
        fixed.push(line);
    }
    let mut text = fixed.join("\n");
    text.push('\n');
    text
}

/// The next line of the original, or a failure naming what ran out.
fn next<'a>(lines: &mut Lines<'a>, what: &str) -> &'a str {
    lines
        .next()
        .unwrap_or_else(|| panic!("{what} does not apply: it runs past the end of the fixture"))
}

/// The hunk `line` opens, where it is a hunk header.
///
/// A header the diff carries but this does not recognise would
/// silently become a body line, so the shape is required in full:
/// `@@ -` then the start, then whatever the counts and the new side
/// spell, then ` @@`.
fn take_hunk_header(line: &str) -> Option<Hunk> {
    let rest = line.strip_prefix("@@ -")?;
    let (start, rest) = take_number(rest)?;
    assert!(
        rest.contains(" @@"),
        "a hunk header is `@@ -<start>,<count> +<start>,<count> @@`, got {line:?}",
    );
    Some(Hunk { start })
}

/// The decimal number `text` opens with, and what follows it.
fn take_number(text: &str) -> Option<(usize, &str)> {
    let digits = text.len() - text.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    match digits {
        0 => None,
        _ => Some((
            text[..digits].parse().expect("digits parse"),
            &text[digits..],
        )),
    }
}

/// The marker `line` opens with and the text after it, where the line
/// is a hunk body line.
///
/// A diff's two header lines are `---` and `+++`, which open with a
/// marker and would otherwise read as a removal and an addition, so
/// they are answered for here rather than skipped by counting lines.
/// `\ No newline at end of file` cannot appear: every fixture ends in
/// one, which `read_text` holds them to.
fn split_marker(line: &str) -> Option<(char, &str)> {
    if line.starts_with("--- ") || line.starts_with("+++ ") {
        return None;
    }
    assert!(
        !line.starts_with('\\'),
        "a fixture ends in a newline, so its diff carries no {line:?}",
    );
    match line.chars().next() {
        Some(marker @ (' ' | '-' | '+')) => Some((marker, &line[1..])),
        // `git diff` writes a bare empty line for an unchanged empty
        // line, the trailing space having been stripped on the way
        // into the file.
        None => Some((' ', "")),
        // Anything else is a line of the `diff --git` preamble, which
        // a diff recorded with it left on carries.
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests;
