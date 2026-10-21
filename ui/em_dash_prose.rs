// Bad: a line comment with an em dash — and a clause after it.
// Bad: a line comment with an en dash – and a clause after it.
// Bad: a line comment with ASCII hyphens -- and a clause after it.
// Good: a line comment with a colon: the clause after it is named.
// Not flagged: a well-known hyphenated word carries one hyphen only.
// Not flagged: the flag --help has no space after its hyphens.
// Not flagged: an HTML comment opens with <!-- and closes with --> markers.
// Not flagged: a quotation "reproduces a dash — as written" verbatim.
// Not flagged: a quotation 'reproduces a dash — as written' verbatim.
// Not flagged: a code span `cargo dylint -- --all-targets` holds a separator.

/* Bad: a block comment with an em dash — and a clause after it. */
/* Good: a block comment with a colon: the clause after it is named. */

/*
 * Bad: the first line of a block comment with an em dash — here.
 * Bad: a later line of a block comment with an em dash — here.
 */

/// Bad: a doc comment with an em dash — and a clause after it.
fn _doc_em_dash() {}

/// Bad: the first line of a doc comment with an em dash — here.
/// Bad: a later line of a doc comment with an em dash — here.
fn _doc_several_lines() {}

/// Good: a doc comment with a colon: the clause after it is named.
fn _doc_colon() {}

/** Bad: a `/** */` doc comment with an em dash — and a clause. */
fn _doc_block() {}

/// Not flagged: an inline code span `a — b` holds quoted code.
fn _doc_code_span() {}

/// Not flagged: a fenced block holds code, so a Lua comment marker stays.
///
/// ```lua
/// local x = 1 -- a comment
/// ```
fn _doc_fenced_block() {}

/// Not flagged: an indented block holds code, so a separator stays.
///
///     cargo dylint -- --all-targets
fn _doc_indented_block() {}

/// Not flagged: a blockquote reproduces words the author did not write.
///
/// > A quotation keeps the dash — it was written with.
fn _doc_blockquote() {}

/// Bad: an alert is an aside in the author's own voice, so the dash
/// inside one is theirs to restructure.
///
/// > [!NOTE]
/// > An alert carries a remark — and a clause after it.
fn _doc_alert() {}

mod inner {
    //! Bad: an inner doc comment with an em dash — and a clause.
}

fn main() {
    // Bad: a plain comment is not markdown, so its `>` opens no
    // blockquote and the dash in the `>` line is reported.
    // > A line comment is not markdown — and holds no quotation.
    let _ = "Not flagged: a plain string literal holds a dash — unscanned.";
}
