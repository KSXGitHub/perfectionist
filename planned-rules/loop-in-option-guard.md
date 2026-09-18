# `loop_in_option_guard`

**Source:** project convention.

## Statement

An `Option` is already an iterator of at most one item. Unwrapping
one with `if let Some(..)` only to iterate the binding says that
twice, and pays a level of indentation for the repetition:

```rust
if let Some(targets) = manifest.get("target").and_then(Value::as_table) {
    for (cfg, target) in targets {
        collect(target, cfg);
    }
}
```

`into_iter().flatten()` is the same control flow with the absent
case folded into the iterator, where it belongs — no items to
iterate:

```rust
for (cfg, target) in manifest
    .get("target")
    .and_then(Value::as_table)
    .into_iter()
    .flatten()
{
    collect(target, cfg);
}
```

## Why restrict this?

This is a stylistic preference, not a correctness issue. Both forms
run the same iterations and neither is faster. The objection is to
the shape:

- **The guard adds a level of indentation that carries no decision.**
  Nothing happens in the `None` case, so the `if` exists only to
  reach the loop. A reader tracking nesting has to descend a level
  and then discover there was nothing to decide.
- **It separates the option handling from the thing it guards.** The
  scrutinee sits one line above the iteratee it *is*, so the
  relationship between them has to be reconstructed.
- **It does not compose.** Each additional optional layer adds
  another `if let` and another level, where the adaptor chain stays
  flat.

## What to lint

An `if let Some(binding) = scrutinee { .. }` expression where:

1. There is **no `else` branch**. An `else` makes the absent case a
   decision, which is what the rule says this shape lacks.
2. The block's **only statement** is a `for` loop, and the loop's
   iteratee is exactly the `binding` — a bare path to it, not an
   expression derived from it. `for x in binding.values()` is a
   different shape and is left alone.
3. The `if let` is in **statement position** and its value is unused,
   so replacing it with a loop is type-correct.
4. The binding is **not used after the loop** inside the block —
   condition 2 already implies the block has one statement, so this
   holds by construction; assert it rather than re-deriving it.

Emit one suggestion: replace the whole `if let` with the `for`,
splicing `.into_iter().flatten()` onto the scrutinee. It is
`MachineApplicable` — the rewrite is total and the iteration order
and count are unchanged.

`if let Some(x) = opt` where the loop iterates something *derived*
from `x` does not fire, nor does a `while let`, nor a `match` with a
`None => {}` arm. Each is a different shape, and widening to them
would claim more than the name does.

## Examples

### The shape

**Avoid:**

```rust
if let Some(entries) = section {
    for entry in entries {
        record(entry);
    }
}
```

**Prefer:**

```rust
for entry in section.into_iter().flatten() {
    record(entry);
}
```

### Not flagged

```rust
// An `else` branch makes the absent case a decision.
if let Some(entries) = section {
    for entry in entries { record(entry); }
} else {
    record_missing();
}

// The loop iterates something derived from the binding.
if let Some(entries) = section {
    for entry in entries.values() { record(entry); }
}

// The block does more than loop.
if let Some(entries) = section {
    note(entries.len());
    for entry in entries { record(entry); }
}
```

## Configuration

The rule takes no configuration. The trigger is one shape and the
rewrite has one form, so there is nothing to turn off short of the
rule itself.

Suppress a site with `#[expect(perfectionist::loop_in_option_guard)]`,
or the whole rule through the `[perfectionist]` global table:

```toml
[perfectionist]
disable = ["loop_in_option_guard"]
```

## Implementation notes

- **Pass kind.** `LateLintPass::check_expr` on `ExprKind::If` whose
  condition is an `ExprKind::Let`. Types are not strictly needed to
  match the shape, but they are needed to confirm the scrutinee is an
  `Option` rather than another type with a `Some`-like variant, so a
  late pass it is.
- **Matching the body.** The `then` block must have exactly one
  statement and no tail expression — or no statements and a `for`
  tail expression. A `for` desugars to a `loop` inside a `match` in
  HIR, so match on the pre-desugaring form through
  `higher::ForLoop::hir`, which `clippy_utils` provides for exactly
  this.
- **Matching the iteratee.** Resolve the loop's iteratee to a path
  and compare its `Res` against the `HirId` the `Some` pattern binds.
  Comparing spans or names is not enough — a shadowing binding of the
  same name would match textually.
- **Suggestion span.** Replace the whole `if let` expression.
  `.into_iter().flatten()` binds tighter than most things, but the
  scrutinee still needs parenthesising when it is not already a call
  chain or path; take it through
  `clippy_utils::source::snippet_with_applicability` and wrap when
  `expr.precedence()` calls for it.
- **`&` versus owned.** The scrutinee is usually `Option<&T>` where
  `&T: IntoIterator`, but an `Option<T>` iterated by reference needs
  `.iter().flatten()` rather than `.into_iter().flatten()` to keep
  borrowing rather than moving. Pick the adaptor from the loop's own
  desugared iteratee (`IntoIterator::into_iter` receiver type), not
  from the scrutinee alone.
- **Proc-macro suppression.** The primary span is the whole `if let`,
  wider than the synthesised spans the
  [suppression convention](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations)
  warns about, so `report_in_external_macro: false` should suffice.
  Confirm against a `ui/loop_in_option_guard_proc_macro.rs` fixture
  and record the outcome at the span-selection site.
- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for the cross-cutting conventions that apply to every rule here.

### Difficulty

**Easy.** A closed HIR shape, one type check, one textual rewrite,
no configuration. The only real care is the `iter` versus
`into_iter` choice above, which is decided by the loop's own
desugaring rather than guessed.

## Default state

Active by default. The shape has one direction and the rewrite is
total, so there is no baseline a project could reasonably prefer and
no threshold to tune.

## Interaction with sibling rules

- [`option-guard-in-loop`](./option-guard-in-loop.md) is the mirror
  image — an `if let` *inside* a `for` rather than around it — and
  the two compose on the shape that nests both, each firing on its
  own level.
- No Clippy lint covers this. With `clippy::all`, `clippy::pedantic`
  and `clippy::nursery` enabled together, the shape above produces no
  diagnostic.
- `perfectionist::excessive_nesting` counts depth without judging
  what produced it; this rule removes one specific cause. A body deep
  enough to trigger both triggers both.
