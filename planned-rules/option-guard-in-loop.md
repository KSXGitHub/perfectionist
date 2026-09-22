# `option_guard_in_loop`

**Source:** project convention.

## Statement

A `for` loop whose entire body is an `if let Some(..)` iterates
items it then discards. The discarding is the iterator's job, and
`filter_map` is where it says so:

```rust
for (cfg, target) in targets {
    if let Some(table) = target.as_table() {
        collect(table, cfg);
    }
}
```

```rust
for (cfg, table) in targets.filter_map(|(cfg, target)| Some((cfg, target.as_table()?))) {
    collect(table, cfg);
}
```

The loop body is then left saying what it does, rather than which
condition had to hold for it to run at all.

## Why restrict this?

This is a stylistic preference, not a correctness issue. Both forms
do the same work in the same order. The objection is to where the
selection lives:

- **Selection and work are interleaved.** The reader meets the body
  before learning which items reach it, and has to hold the
  condition in mind while reading past it.
- **The indentation overstates the body.** Every line of real work
  sits one level deeper than the loop that contains it, for a
  condition that contributes nothing to the body.
- **It hides how many items the loop actually processes.** `for x in
  it` reads as "every item"; the count is only apparent once the
  guard inside is read.

## What to lint

A `for` loop whose body block has **exactly one statement and no
tail expression**, that statement being an `if let Some(binding) =
scrutinee { .. }` with **no `else`**, where:

1. The scrutinee is an expression **derived from the loop binding** —
   it mentions the loop's pattern bindings. A scrutinee independent of
   the loop binding is loop-invariant and belongs outside the loop
   entirely, which is a different complaint and not this rule's.
2. The scrutinee is **not the loop binding itself**. That degenerate
   case — where the element already *is* the `Option` — is
   `clippy::manual_flatten`'s, and its `flatten()` suggestion is
   better than the `filter_map(|x| x)` this rule would produce. Defer
   to it rather than duplicate it.
3. The body contains no `break`, `continue`, `return` or `?` that
   would escape the loop. Each changes meaning once the body moves
   into a closure, and none can be expressed there.

Emit one suggestion: move the scrutinee into a `filter_map` on the
iteratee, rebinding the loop pattern to carry the unwrapped value
alongside whatever else the pattern bound.

Applicability is **`MaybeIncorrect`**: the rewrite moves an
expression into a closure, so it can change when a borrow is taken
even where it cannot change the result. Offer it, do not apply it.

## Examples

### The shape

**Avoid:**

```rust
for entry in entries {
    if let Some(name) = entry.file_name() {
        record(name);
    }
}
```

**Prefer:**

```rust
for name in entries.filter_map(|entry| entry.file_name()) {
    record(name);
}
```

### Carrying more than the unwrapped value

**Avoid:**

```rust
for (cfg, target) in targets {
    if let Some(table) = target.as_table() {
        collect(table, cfg);
    }
}
```

**Prefer:**

```rust
for (cfg, table) in targets.filter_map(|(cfg, target)| Some((cfg, target.as_table()?))) {
    collect(table, cfg);
}
```

### Not flagged

```rust
// The element already is the `Option`: `clippy::manual_flatten`'s case.
for item in items {
    if let Some(value) = item { record(value); }
}

// Control flow that cannot move into a closure.
for entry in entries {
    if let Some(name) = entry.file_name() {
        if name.is_empty() { break; }
        record(name);
    }
}

// The body does more than the guard.
for entry in entries {
    count += 1;
    if let Some(name) = entry.file_name() { record(name); }
}

// The scrutinee does not depend on the loop binding.
for entry in entries {
    if let Some(root) = shared_root { record(root, entry); }
}
```

## Configuration

The rule takes no configuration. The trigger is one shape and the
rewrite has one form.

Suppress a site with `#[expect(perfectionist::option_guard_in_loop)]`,
or the whole rule through the `[perfectionist]` global table:

```toml
[perfectionist]
disable = ["option_guard_in_loop"]
```

## Implementation notes

- **Pass kind.** `LateLintPass::check_expr`, matching the loop
  through `higher::ForLoop::hir` so the pre-desugaring shape is
  available rather than the `loop`/`match` it lowers to.
- **Body shape.** One statement, no tail expression, and that
  statement an `ExprKind::If` with an `ExprKind::Let` condition and
  no `else`.
- **Dependence on the loop binding.** Walk the scrutinee for a path
  whose `Res` resolves to one of the loop pattern's `HirId`s; a
  `clippy_utils` visitor does this. Bail when none is found
  (loop-invariant) and when the scrutinee *is* exactly that path
  (Clippy's case).
- **Escaping control flow.** Walk the body for `ExprKind::Break`,
  `ExprKind::Continue`, `ExprKind::Ret` and the `?` desugaring, and
  bail on any. `break`/`continue` targeting a *nested* loop inside
  the body are fine; compare the target label against the loop being
  rewritten rather than rejecting on the node kind alone.
- **Building the closure.** The closure parameter is the loop
  pattern's own source text, and the body is
  `Some((<bound names…>, <scrutinee>?))` — or just `<scrutinee>` when
  the pattern binds exactly the one name the guard consumes. Both
  forms come from snippets, so a pattern that does not round-trip
  through its own source text (an elided struct pattern, say) is a
  reason to bail.
- **Proc-macro suppression.** The primary span is the whole loop,
  wider than the synthesised spans the
  [suppression convention](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations)
  warns about, so `report_in_external_macro: false` should suffice.
  Confirm against a `ui/option_guard_in_loop_proc_macro.rs` fixture.
- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for the cross-cutting conventions that apply to every rule here.

### Difficulty

**Medium.** Detection is as cheap as its sibling's, but the fix is
not: it moves an expression into a closure, so it has to reason
about escaping control flow, about which names the pattern binds and
must carry through, and about patterns whose source text cannot be
reused. Most of the work is in deciding when *not* to suggest.

## Default state

Active by default, but suggestion-only — the diagnostic is
`MaybeIncorrect`, so `cargo dylint --fix` leaves the choice to the
author rather than rewriting a loop body into a closure unattended.

## Interaction with sibling rules

- [`loop-in-option-guard`](./loop-in-option-guard.md) is the mirror
  image — an `if let` *around* a `for` rather than inside it. A shape
  that nests both triggers both, each on its own level.
- `clippy::manual_flatten` covers the degenerate case where the loop
  element itself is the `Option`, and this rule skips it by design
  (see [What to lint](#what-to-lint)). Outside that case Clippy is
  silent: with `clippy::all`, `clippy::pedantic` and
  `clippy::nursery` enabled together, a guard whose scrutinee is a
  call on the element produces no diagnostic. This rule is that
  lint's extension rather than a refinement, which is why it does not
  borrow the name.
- `perfectionist::excessive_nesting` counts depth without judging
  what produced it; this rule removes one specific cause.
