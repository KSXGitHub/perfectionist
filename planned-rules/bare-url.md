# `bare_url`

**Source:** project convention.

## Statement

In doc comments (`///`, `//!`) and regular comments (`//`, `/* */`),
URLs written as bare text — `https://example.com` — must be turned
into one of:

- Markdown autolink: `<https://example.com>`.
- Markdown labelled link: `[Example](https://example.com)`.

Bare URLs rely on autolinkification, which is inconsistent across
renderers: rustdoc renders them, GitHub-flavored markdown renders
them, but plain CommonMark does not. Wrapping in `<>` is the
portable form and signals the author's intent explicitly.

## What to lint

Scan every doc comment and regular comment for the regex equivalent of
`(?<![<\[(])\bhttps?://\S+` (an `http://` or `https://` followed by
non-whitespace, not already preceded by `<`, `[`, or `(`).

For each match outside a code span / code block, emit a diagnostic at
the URL span.

Skip:

- Inside `` `...` `` code spans and ` ``` ... ``` ` fences.
- URLs already inside `<...>`.
- URLs already inside `[label](url)` or `[label][id]` followed by
  `[id]: url`.
- URLs that are themselves the *target* of a reference-link
  definition (`[id]: https://...`).

## Examples

```rust
// Bad
/// See https://example.com for details.

// Good (autolink)
/// See <https://example.com> for details.

// Good (labelled)
/// See [the example site](https://example.com) for details.
```

## Autofix

The autofix wraps the bare URL with `<` and `>`. Applicability
depends on the URL's last character, which determines whether
trailing punctuation belongs to the URL or to the surrounding
sentence:

- **`MachineApplicable`** when the URL ends with one of:
  - `/` (path slash, unambiguous),
  - any ASCII alphanumeric,
  - `_`, `-`, `=`, `&`, `+` (URL-safe characters).
- **`MaybeIncorrect`** otherwise — most importantly when the URL
  appears to end with `.`, `?`, `!`, `,`, `;`, `:`, `)`, `]`, `>`,
  `'`, or `"`. Those characters are syntactically valid in a URL but
  far more often punctuate the surrounding sentence. The author
  needs to decide whether the trailing character is part of the URL
  or not, so the lint emits a help-only suggestion that highlights
  both interpretations.

The lint never strips trailing punctuation on the author's behalf.

## Examples of autofix applicability

```rust
// MachineApplicable (ends in /)
/// See https://example.com/ for details.
//      ^^^^^^^^^^^^^^^^^^^^^
// → <https://example.com/>

// MachineApplicable (ends in alphanumeric)
/// See https://example.com/path
//      ^^^^^^^^^^^^^^^^^^^^^^^^
// → <https://example.com/path>

// MaybeIncorrect (ends in .)
/// See https://example.com.
//      ^^^^^^^^^^^^^^^^^^^^
// Two interpretations: the URL might end in `.com` or `.com.` (the
// former with a sentence period after). The lint shows both.

// MaybeIncorrect (ends in ?)
/// Have you seen https://example.com?
//                ^^^^^^^^^^^^^^^^^^^^^
```

## Configuration

```toml
[bare_url]
# Where the lint applies.
targets = ["doc", "comment"]    # subset of these two

# Which forms count as compliant.
accept = ["angle_brackets", "labelled"]
# `["angle_brackets"]` only requires `<...>` (rejects `[label](url)`).
# `["labelled"]`       only requires `[label](url)`.
# Both means either form is accepted.

# Characters that, when the URL ends in one of them, qualify the
# autofix as MachineApplicable. Defaults match the list above.
safe_trailing_chars = ["/", "_", "-", "=", "&", "+"]

# When false, restrict the URL pattern to https only.
allow_http = true

# Skip URLs whose host matches any of these patterns. Useful for
# placeholder hosts that should remain bare in docs.
skip_hosts = ["example.com", "example.org", "localhost"]
```

## Implementation notes

- `LateLintPass`. Reuse the doc-comment scanner from
  [`intra-doc-links`](./intra-doc-links.md) for the doc target and
  the regular-comment retokenizer from
  [`unicode-ellipsis-in-comments`](./unicode-ellipsis-in-comments.md)
  for the comment target.
- The autofix span includes only the matched URL bytes; the
  replacement is `<{matched}>`. For the `MaybeIncorrect` cases,
  emit two suggestions — one keeping the trailing character inside
  `<...>`, one moving it outside — so the author can pick.
- The match is greedy on `\S+` but stops before a closing bracket
  or angle bracket that would invalidate the wrap.
- **Parser style.** Implement URL discovery as parser-combinator-
  style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
  `take_scheme` (`http://` / `https://`), `take_authority`,
  `take_path`, and a `take_trailing_punctuation` helper that
  classifies the last byte as either part of the URL or part of the
  surrounding sentence. The trailing-punctuation decision is the
  reason this rule wants combinators rather than a single regex —
  the caller's choice of when to commit drives the
  `MachineApplicable` vs `MaybeIncorrect` split.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name prefixing (`perfectionist_*`)
  required for every registered lint.

## Severity

Warn.
