# `some_bool_comparison`

**Source:** project convention.

## Statement

An `Option<bool>` has three states. Comparing one against a `Some`
of a boolean literal collapses all three with an operator, leaving
the reader to work out which states the comparison admits:

```rust
if spec.get("workspace").and_then(Value::as_bool) == Some(true) {
```

The standard library has a method that says it instead. `None` is
one of the two answers, and naming the answer it gives is the whole
content of the fix:

```rust
if spec.get("workspace").and_then(Value::as_bool).unwrap_or(false) {
```

Each of the four comparisons has a `unwrap_or` form, taking the
*opposite* literal as the default and negated when the operator and
the literal disagree:

| Comparison            | Reads as              | Prefer                 |
|-----------------------|-----------------------|------------------------|
| `opt == Some(true)`   | present and true      | `opt.unwrap_or(false)` |
| `opt != Some(false)`  | absent or true        | `opt.unwrap_or(true)`  |
| `opt != Some(true)`   | absent or false       | `!opt.unwrap_or(false)`|
| `opt == Some(false)`  | present and false     | `!opt.unwrap_or(true)` |

## Why restrict this?

This is a stylistic preference, not a correctness issue. The
comparison is exactly equivalent to its replacement and compiles to
the same thing. The objection is to what the reader has to do:

- **The absent case is left implicit.** `== Some(true)` never says
  what `None` means, so the reader derives it from the operator.
  `unwrap_or(false)` states it — absent counts as false — which is
  usually the decision the surrounding code actually turned on.
- **`!=` compounds that.** `opt != Some(true)` admits two states for
  two different reasons, and which two is not apparent without
  working through the cases.
- **It reads as a value test, not a presence test.** Nothing at the
  call site distinguishes `Option<bool>` from `bool`, so a reader
  scanning for the option-handling in a function can pass straight
  over it.

## What to lint

A binary `==` or `!=` where one operand is an expression of type
`Option<bool>` and the other is `Some(<boolean literal>)`, in either
order.

1. Resolve the operand types through `cx.typeck_results()`. One side
   must be `Option<bool>` after adjustment; a comparison between two
   `Option<bool>` *expressions* is not in scope, because neither side
   names a state to describe.
2. The literal side must be a call to `Option::Some` — resolved
   through the path, not by the name `Some` — whose single argument
   is a boolean literal. A `Some(flag)` carrying a variable is left
   alone: there is no state to name and `unwrap_or` would not be an
   improvement.
3. Emit one suggestion, chosen by the table above:
   `unwrap_or(!literal)`, wrapped in `!` when the operator being `==`
   disagrees with the literal. It is `MachineApplicable` — the
   rewrite is total and value-preserving.

The rule does **not** fire on `matches!(opt, Some(true))`. That form
already names the state it matches, which is what this rule is
asking for.

Nor does it need a `const` exemption. Neither shape is available in
a `const fn` today: `PartialEq` is not yet a const trait, so the
comparison this rule fires on cannot appear there in the first place.

## Examples

### The shape, in each operator and literal combination

**Avoid:**

```rust
let verbose = flags.get("verbose").and_then(Value::as_bool);

if verbose == Some(true) { /* ... */ }
if verbose != Some(true) { /* ... */ }
if verbose == Some(false) { /* ... */ }
if verbose != Some(false) { /* ... */ }
```

**Prefer:**

```rust
let verbose = flags.get("verbose").and_then(Value::as_bool);

if verbose.unwrap_or(false) { /* ... */ }
if !verbose.unwrap_or(false) { /* ... */ }
if !verbose.unwrap_or(true) { /* ... */ }
if verbose.unwrap_or(true) { /* ... */ }
```

### Reversed operands

**Avoid:**

```rust
if Some(true) == entry.enabled { /* ... */ }
```

**Prefer:**

```rust
if entry.enabled.unwrap_or(false) { /* ... */ }
```

### Not flagged

```rust
// Both sides are `Option<bool>`: neither names a state.
if left == right { /* ... */ }

// The `Some` carries a variable, not a literal.
if enabled == Some(wanted) { /* ... */ }

// Not an `Option<bool>`.
if version == Some(3) { /* ... */ }

// Already names the state it matches.
if matches!(enabled, Some(true)) { /* ... */ }
```

## Configuration

The rule takes no configuration. The trigger admits no variants
worth turning off, and the fix has one form per operator-and-literal
pair with nothing to choose between.

Suppress a site with `#[expect(perfectionist::some_bool_comparison)]`,
or the whole rule through the `[perfectionist]` global table:

```toml
[perfectionist]
disable = ["some_bool_comparison"]
```

## Implementation notes

- **Pass kind.** `LateLintPass::check_expr` on `ExprKind::Binary`
  with `BinOpKind::Eq` or `BinOpKind::Ne`. Types are required, so
  this cannot be an early pass.
- **Recognising the `Some`.** Match `ExprKind::Call` whose callee is
  a path resolving to `Option::Some` — via `cx.qpath_res` and a
  `LangItem`/diagnostic-item check rather than the textual name, so
  a local `enum Mine { Some(bool) }` does not match. The argument is
  `ExprKind::Lit` of `LitKind::Bool`.
- **Recognising the option side.** `cx.typeck_results().expr_ty_adjusted`
  must be `Option<bool>`; check the ADT is the `Option` diagnostic
  item and its single generic argument is `bool`.
- **Suggestion span.** Replace the whole binary expression, taking
  the option side's snippet through
  `clippy_utils::source::snippet_with_applicability`. Parenthesise
  the option side when it is not already a place or call chain, so
  `!a && b == Some(true)` does not rewrite into something that
  reassociates.
- **Proc-macro suppression.** The diagnostic's primary span is the
  whole binary expression, wider than the synthesised spans the
  [suppression convention](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations)
  warns about, so `report_in_external_macro: false` should suffice.
  Confirm that against a `ui/some_bool_comparison_proc_macro.rs`
  fixture before relying on it, and record the outcome at the
  span-selection site either way.
- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for the cross-cutting conventions that apply to every rule here,
  in particular the `perfectionist::` lint-name namespacing.

### Difficulty

**Easy.** One expression shape, one type check, four fixed rewrites
and no configuration. The only subtlety is parenthesising the option
side in the suggestion.

## Default state

Active by default. The trigger is one unambiguous expression shape
with a total, value-preserving rewrite, so there is no baseline a
project could reasonably want and no threshold to tune. A project
that disagrees turns it off in one line rather than configuring it.

## Interaction with sibling rules

- `clippy::bool_comparison` is the adjacent lint, and it does not
  cover this. It rewrites `b == true` to `b` for a plain `bool` and
  stops there: with `clippy::all`, `clippy::pedantic`,
  `clippy::nursery` and `clippy::restriction` all enabled, none of
  `opt == Some(true)`, `opt == Some(false)` or `opt != Some(true)`
  produces a diagnostic. This rule is that lint's complement on
  `Option<bool>` rather than a refinement of it, which is why it does
  not borrow the name.
