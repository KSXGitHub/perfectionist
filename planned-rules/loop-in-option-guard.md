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
iterate. The scrutinee keeps the name the guard gave it, so the
`for` header stays one line:

```rust
let targets = manifest.get("target").and_then(Value::as_table);
for (cfg, target) in targets.into_iter().flatten() {
    collect(target, cfg);
}
```

Folding the scrutinee into the header instead is right only where it
is already one short expression — `for x in opt.into_iter().flatten()`.
A chained one does not fit: rustfmt breaks the header across a line
per call and leaves the brace on its own, which spends more lines
than the level it saves.

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

Emit no suggestion. Which rewrite is right depends on the
scrutinee, as the [Statement](#statement) shows — folded into the
header where it is one short expression, bound to a `let` where it is
a chain — and that is a judgement about the surrounding code rather
than a fact about the shape. A `MachineApplicable` fix would be
applied unreviewed by `--fix`, and on a chained scrutinee it makes
the code worse; rustfmt then bakes the result in. So the diagnostic
carries help naming both forms and why the second exists, in the
shape `perfectionist::overly_long_method_chain` uses:

> **help:** an `Option` is already an iterator of at most one item —
> fold the absent case into the iterator with
> `.into_iter().flatten()` where the scrutinee is one short
> expression
>
> **help:** where it is a chain, bind it to a `let` first and iterate
> the binding — folding it into the `for` header splits the header
> across a line per call and adds two calls toward
> `overly_long_method_chain`'s limit

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
- **Diagnostic span.** The whole `if let` expression. Emitting no
  rewrite means no snippet to reuse, so neither the parenthesising
  nor the `iter`-versus-`into_iter` question below arises in the
  output — but the help text still names `.into_iter().flatten()`, so
  say `iter()` instead where the loop borrows. Read that from the
  loop's own desugared iteratee (`IntoIterator::into_iter` receiver
  type) rather than from the scrutinee alone: the scrutinee is
  usually `Option<&T>` where `&T: IntoIterator`, but an `Option<T>`
  iterated by reference needs `.iter().flatten()` to keep borrowing
  rather than moving.
- **Proc-macro suppression.** The primary span is the whole `if let`,
  wider than the synthesised spans the
  [suppression convention](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations)
  warns about, so `report_in_external_macro: false` should suffice.
  Confirm against a `ui/loop_in_option_guard_proc_macro.rs` fixture
  and record the outcome at the span-selection site.
- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for the cross-cutting conventions that apply to every rule here.

### Difficulty

**Easy**, and cheaper for carrying no rewrite: a closed HIR shape,
one type check, no snippet reuse, no configuration. The only real
care is the `iter` versus `into_iter` wording above, which is decided
by the loop's own desugaring rather than guessed.

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
