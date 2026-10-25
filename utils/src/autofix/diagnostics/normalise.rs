//! Take the churn out of one rendered diagnostic.
//!
//! Each rule here answers something that differs between two runs of
//! the same unchanged project, or between two machines, and so would
//! have a stored expectation rewritten by a change that is not the
//! one under review.
//!
//! A `.stderr` under `ui/` is normalised by compiletest directives
//! over output rustc rendered under `-Zui-testing`, which is what
//! turns its gutters into `LL`. The checks here run without that
//! flag, so that rustc still sees the same command line as the fix
//! pass before it and the check costs nothing to repeat. What the
//! flag would have done is done below instead, which is why the
//! gutter is rebuilt rather than only renumbered: rustc sizes the
//! gutter column by the widest line number in the block, so a
//! diagnostic moving from line 9 to line 10 re-indents every line of
//! it.
//!
//! The shapes worth recognising are taken by the `take_*` prefix
//! parsers the conventions file calls for. None asserts that its
//! input is exhausted; the rewriter that owns the line decides where
//! the token it took had to end.

/// The gutter column every block is rebuilt around, which is the
/// width of the `LL` that replaces a line number.
const GUTTER: usize = 2;

/// Normalise one diagnostic as rustc rendered it. `project_dir` is
/// the directory the check ran in, spelled as the diagnostic spells
/// it.
pub(super) fn diagnostic(rendered: &str, project_dir: &str) -> String {
    rendered
        .replace(project_dir, "$PROJECT")
        .lines()
        .map(without_colour)
        .map(|line| anonymise_file_positions(&line))
        .map(|line| rebuild_gutter(&line))
        .map(|line| line.trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_owned()
}

/// Rewrite every `.rs:<line>:<column>` as `.rs:LL:CC`, so inserting
/// a line above a diagnostic does not renumber the span headers below
/// it. Anchoring on the extension is what the `ui/` sweep does, and
/// is what keeps a path that merely holds a colon intact.
fn anonymise_file_positions(line: &str) -> String {
    const SOURCE: &str = ".rs";
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(index) = rest.find(SOURCE) {
        let (before, after) = rest.split_at(index + SOURCE.len());
        out.push_str(before);
        match take_line_and_column(after) {
            Some(((), tail)) => {
                out.push_str(":LL:CC");
                rest = tail;
            }
            None => rest = after,
        }
    }
    out.push_str(rest);
    out
}

/// Re-emit a line's gutter at [`GUTTER`] columns, leaving everything
/// after the marker alone. A line carrying no gutter comes back as it
/// went in.
///
/// Every line of a block is re-indented by the same amount, so the
/// carets rustc aligned under a quoted source line stay under it.
fn rebuild_gutter(line: &str) -> String {
    match take_gutter(line) {
        Some(((number, marker), rest)) => {
            let number = match number.is_empty() {
                true => " ".repeat(GUTTER),
                false => "LL".to_owned(),
            };
            format!("{number} {marker}{rest}")
        }
        None => rebuild_indent(line),
    }
}

/// Re-indent a line that carries no gutter of its own but is indented
/// against one: a span header, whose arrow sits a column left of the
/// marker, and a note, which lines up with it.
fn rebuild_indent(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.starts_with("-->") {
        return format!("{}{trimmed}", " ".repeat(GUTTER));
    }
    if trimmed.starts_with('=') {
        return format!("{}{trimmed}", " ".repeat(GUTTER + 1));
    }
    line.to_owned()
}

/// Take a gutter: the line number, blank on every row but a quoted
/// one, then the column's marker.
///
/// A `-` marker is what tells a gutter from a span header, so a
/// deletion row has to carry a number and a bare `  --> ` must not
/// read as one.
fn take_gutter(input: &str) -> Option<((&str, char), &str)> {
    let marker_at = input.find(['|', '-', '+'])?;
    let (number, rest) = input.split_at(marker_at);
    if !number
        .chars()
        .all(|found| found == ' ' || found.is_ascii_digit())
    {
        return None;
    }
    let marker = rest.chars().next()?;
    let rest = &rest[marker.len_utf8()..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    match (marker, number.trim().is_empty()) {
        ('|', _) => Some(((number.trim(), marker), rest)),
        (_, false) => Some(((number.trim(), marker), rest)),
        (_, true) => None,
    }
}

/// Take a `:<line>:<column>`, each a run of digits.
fn take_line_and_column(input: &str) -> Option<((), &str)> {
    let ((), rest) = take_digits(input.strip_prefix(':')?)?;
    let ((), rest) = take_digits(rest.strip_prefix(':')?)?;
    Some(((), rest))
}

/// Take a run of at least one ASCII digit.
fn take_digits(input: &str) -> Option<((), &str)> {
    let end = input
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(input.len());
    match end {
        0 => None,
        _ => Some(((), &input[end..])),
    }
}

/// Drop the ANSI escapes cargo renders a diagnostic with where it
/// takes its output for a terminal. Which it takes it for is not
/// something a stored expectation should turn on.
fn without_colour(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(index) = rest.find('\u{1b}') {
        out.push_str(&rest[..index]);
        let Some(((), after)) = take_escape(&rest[index..]) else {
            return out;
        };
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Take one ANSI escape sequence, which runs from the escape byte to
/// the `m` that closes it.
fn take_escape(input: &str) -> Option<((), &str)> {
    let rest = input.strip_prefix('\u{1b}')?;
    let end = rest.find('m')?;
    Some(((), &rest[end + 1..]))
}

#[cfg(test)]
mod tests;
