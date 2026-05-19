# `bare_email`

**Default state:** `active`  
**Source:** project convention.

## Statement

In doc comments (`///`, `//!`) and regular comments (`//`, `/* */`),
email addresses written as bare text — `user@example.com` — must be
wrapped according to the project's chosen style. The available styles
are:

- **`angle_brackets`** — `<user@example.com>`.
- **`mailto`** — `mailto:user@example.com`.
- **`both`** — `<mailto:user@example.com>`.
- **`either`** (default) — accept any of the wrapped forms above.
- **`forbid`** — no email addresses allowed in source, in any form.
  For projects that prefer to keep contact information out of the
  repository entirely (privacy posture, or to push contact through
  an external channel like a `CONTRIBUTING.md`).

## What to lint

Scan every doc comment and regular comment for the regex equivalent of
`(?<![<:\w])[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b` (a
local-part / `@` / domain pattern not already preceded by `<`, `:`,
or a word character).

For each match outside a code span / code block, emit a diagnostic
at the email span.

Skip:

- Inside `` `...` `` code spans and ` ``` ... ``` ` fences.
- Already inside `<...>` (with or without `mailto:`).
- Already prefixed with `mailto:` (with or without `<...>`).
- The match is the target of a reference-link definition
  (`[id]: mailto:...`).

## Examples

```rust
// Bad
/// Report security issues to security@example.com.

// Good (angle_brackets style)
/// Report security issues to <security@example.com>.

// Good (mailto style)
/// Report security issues to mailto:security@example.com.

// Good (both style)
/// Report security issues to <mailto:security@example.com>.
```

Under `style = "forbid"`:

```rust
// Bad: any email address, in any form
/// Report security issues to <security@example.com>.

// Good
/// Report security issues through the channel listed in
/// [SECURITY.md](../SECURITY.md).
```

## Autofix

The autofix depends on the configured style:

- `angle_brackets` → wrap with `<` and `>`. `MachineApplicable`.
- `mailto`        → prefix with `mailto:`. `MachineApplicable`.
- `both`          → wrap with `<mailto:` and `>`. `MachineApplicable`.
- `either`        → ambiguous; emit two suggestions
  (`<email>` and `mailto:email`), both `MaybeIncorrect`. The
  author picks which to apply.
- `forbid`        → no autofix; emit help text suggesting that the
  email be moved to an external file or removed entirely.

## Configuration

```toml
[bare_email]
# Required form for compliant email addresses.
style = "either"
# "angle_brackets" | "mailto" | "both" | "either" | "forbid"

# Where the lint applies.
targets = ["doc", "comment"]

# Skip these exact addresses. Useful for `noreply@github.com` and
# similar placeholders that the project deliberately leaves bare in
# changelog entries.
skip_addresses = []

# Skip addresses whose domain matches any of these patterns. Useful
# alongside `skip_addresses` for blanket allowlists.
skip_domains = ["example.com", "example.org"]
```

## Implementation notes

- `LateLintPass`. Share the doc-comment scanner with
  [`intra-doc-links`](./intra-doc-links.md) and the regular-comment
  retokenizer with the implemented
  `perfectionist::unicode_ellipsis_in_comments` lint
  (see `src/lib.rs`).
- The match deliberately requires a top-level domain of at least
  two ASCII letters and a dot before it; this avoids false positives
  on Cargo crate names that happen to contain `@`
  (e.g., `crate@1.2.3` in a lockfile-shaped doc snippet inside a
  code span — though those should already be excluded by the
  code-span filter).
- For `style = "forbid"`, the lint emits no Suggestion. The intent
  is that the address be removed or moved, neither of which is
  mechanically expressible.
- **Parser style.** Implement the address scanner as
  parser-combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md).
  The grammar splits cleanly into `take_local_part` (consumes a run
  of `[A-Za-z0-9._%+-]`), `take_at` (a single `@` byte), and
  `take_domain` (one or more `[A-Za-z0-9-]+` labels separated by
  `.`, with a final TLD of two or more letters). Composing these
  three keeps the email match readable and the failure points
  visible, and avoids dragging a regex engine through the lint
  pass.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.
