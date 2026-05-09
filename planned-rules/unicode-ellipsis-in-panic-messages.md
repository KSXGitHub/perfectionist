# `unicode_ellipsis_in_panic_messages`

**Source:** project convention (parallel to
[`unicode-ellipsis-in-docs`](./unicode-ellipsis-in-docs.md) and
[`unicode-ellipsis-in-comments`](./unicode-ellipsis-in-comments.md)).

## Statement

Forbid U+2026 HORIZONTAL ELLIPSIS (`…`) in the message string of any
panicking or assertion-style macro. Prefer the three-ASCII-dot form
`...`.

Rationale: panic and assertion messages surface in stderr, in CI logs,
in crash reporters, and on terminals whose locale or encoding may not
be UTF-8. Sticking to ASCII means the message renders identically
everywhere.

## What to lint

For every invocation of a panic-family or assertion-family macro,
inspect each string-literal argument that contributes to the message
and flag any U+2026.

Macros in scope (default set):

- `panic!`
- `unimplemented!`
- `todo!`
- `unreachable!`
- `debug_unreachable!` (if present in the crate's macro environment)
- `assert!`, `assert_eq!`, `assert_ne!`
- `debug_assert!`, `debug_assert_eq!`, `debug_assert_ne!`
- `Option::expect`, `Option::unwrap_or_else` (when the closure body is
  a panic-family call), `Result::expect`, `Result::expect_err`

For macros, the message is either the first argument (`panic!`,
`unimplemented!`, `todo!`, `unreachable!`) or a later argument
(`assert!(cond, "message…")` — index 1; `assert_eq!(a, b, "message…")`
— index 2). The lint must look at the relevant argument position for
each macro.

For methods, the receiver is the panicking call site and the message
is the literal argument to `expect`.

## Examples

```rust
// Bad
panic!("could not parse manifest…");

// Good
panic!("could not parse manifest...");
```

```rust
// Bad
let manifest = load().expect("config missing…");

// Good
let manifest = load().expect("config missing...");
```

```rust
// Bad
assert_eq!(actual, expected, "tree did not flatten…");

// Good
assert_eq!(actual, expected, "tree did not flatten...");
```

## Implementation notes

- `LateLintPass::check_expr`. Two cases:
  - **Macro invocations:** detect via `Span::from_expansion` and
    `expn_data.kind` matching `ExpnKind::Macro(_, name)`. Look up the
    macro's `DefId` and match against the diagnostic-named macros
    (`sym::panic_macro`, `sym::assert_macro`, `sym::assert_eq_macro`,
    etc.). For non-diagnostic macros (`todo`, `unimplemented`,
    `unreachable`, `debug_assert*`), fall back to matching by
    fully-qualified path.
  - **Method calls:** match on `ExprKind::MethodCall` with method
    name in `["expect", "expect_err"]` whose receiver type is
    `Option<_>` / `Result<_, _>` (use `clippy_utils::ty::is_type_diagnostic_item`
    with `sym::Option` and `sym::Result`).
- For each match, inspect the relevant argument. If it is a
  `LitKind::Str`, scan the literal contents for U+2026 and emit at
  the byte-offset span.
- Argument that is not a string literal (e.g., a runtime-built
  `format!`-result passed positionally) is out of scope — the lint
  cannot inspect runtime strings. Leave a `// FIXME` note in the
  diagnostic suggesting the user check dynamic messages by inspection.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name prefixing (`perfectionist_*`)
  required for every registered lint.

## Autofix

Replace `…` with `...` inside the literal.
`Applicability::MachineApplicable`.

## Configuration

- `unicode_ellipsis_in_panic_messages.macros` — default list above;
  projects can extend with their own diagnostic macros.
- `unicode_ellipsis_in_panic_messages.methods = ["expect",
  "expect_err"]` — extend with project-specific fallible APIs (e.g.,
  `unwrap_or_panic`).
- `unicode_ellipsis_in_panic_messages.also_flag` — same shape as the
  other two ellipsis rules.

## Severity

Warn.

## Relationship to `em_dash_prose`

[`em-dash-prose`](./em-dash-prose.md) covers a broader set of
user-facing macros (every `format!` / `println!` / log macro) and
targets U+2014. The ellipsis rule is intentionally narrower: it only
covers diagnostic/panic messages, where encoding-portability matters
most. A project that wants ellipsis-banning across *all* output
macros can extend the macro list in this rule's configuration.
