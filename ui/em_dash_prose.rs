// Bad: a line comment is scanned as plain text.
// Rebuilt the index — the previous one was stale.

// Bad: an en dash is reported wherever an em dash is.
// Retrying in a moment – the server was busy.

// Bad: a spaced hyphen run is the typewriter spelling of a dash.
// Wrote the report -- 3 warnings remain.

// Good: a colon names the relationship a dash would have left out.
// Reading the lockfile: this may take a moment.

// Not flagged: a hyphenated word carries one hyphen only.
// The well-known hosts are read from the config.

// Not flagged: a run with no space after it is a flag rather than a
// dash.
// Pass --help to see every option.

// Not flagged: the HTML markers leave no space between the hyphens
// and the angle bracket.
// The template wraps each note in <!-- and --> markers.

// Not flagged: a quotation reproduces someone else's words.
// The log said "build failed — see above" and stopped.

// Not flagged: a single-quoted span is a quotation too.
// The log said 'build failed — see above' and stopped.

// Not flagged: a code span holds code rather than prose.
// Run `cargo dylint -- --all-targets` to check every target.

// Bad: a block comment is scanned as plain text.
/* Walks the tree — and returns the total size. */

// Good: a colon names the relationship a dash would have left out.
/* Reading the lockfile: this may take a moment. */

// Bad: each line of a block comment is scanned.
/*
 * Loads the lockfile — and checks it against the manifest.
 * Counts the entries — the transitive ones included.
 */

// Bad: a doc comment is scanned as markdown.
/// Walks the tree — and returns the total size.
fn _doc_em_dash() {}

// Bad: a hyphen run reaches the doc surface as much as the comment one.
/// Reads the manifest -- and returns its package name.
fn _doc_ascii_run() {}

// Bad: a doc comment's lines are joined, so each dash is its own
// finding.
/// Loads the lockfile — and checks it against the manifest.
/// Counts the entries — the transitive ones included.
fn _doc_several_lines() {}

// Good: a colon names the relationship a dash would have left out.
/// Walks the tree: hidden directories are included too.
fn _doc_colon() {}

// Bad: a `/** */` doc comment is scanned like a `///` one.
/** Walks the tree — and returns the total size. */
fn _doc_block() {}

// Not flagged: an inline code span holds code rather than prose.
/// Formats a span as `1 — 5` for display.
fn _doc_code_span() {}

// Not flagged: a fenced block holds code, so a Lua comment marker
// stays.
/// Evaluates a chunk and hands back its result.
///
/// ```lua
/// local x = 1 -- the starting index
/// ```
fn _doc_fenced_block() {}

// Not flagged: an indented block is a code block as much as a fenced
// one.
/// Checks the workspace with every target enabled.
///
///     cargo dylint -- --all-targets
fn _doc_indented_block() {}

// Not flagged: a blockquote reproduces words the author did not write.
/// Quotes the upstream release note.
///
/// > The parser is stricter now — malformed input is rejected.
fn _doc_blockquote() {}

// Bad: an alert is an aside in the author's own voice, so the dash
// inside one is theirs to restructure.
/// Loads the configuration from disk.
///
/// > [!NOTE]
/// > The profile is read once — later edits are ignored.
fn _doc_alert() {}

// Bad: an inner doc comment is scanned like an outer one.
mod inner {
    //! Rebuilds the index — the previous one was stale.
}

fn main() {
    // Bad: a plain comment is not markdown, so its `>` opens no
    // blockquote and the dash in the `>` line is reported.
    // > The parser is stricter now — malformed input is rejected.

    // Not flagged: a string literal that is no call's message holds
    // nothing the rule scans.
    let _ = "Rebuilt the index — the previous one was stale.";
}
