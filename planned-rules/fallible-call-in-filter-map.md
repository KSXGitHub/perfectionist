# `fallible_call_in_filter_map`

**Source:** project convention, raised in review on
<https://github.com/KSXGitHub/perfectionist/pull/475>. The
workspace-manifest walk in `src/cargo_manifest.rs` was written as two
`filter_map` closures, each performing a fallible call and discarding
its error in the same step.

## Statement

> An iterator adapter should do one thing. A `filter_map` whose closure
> performs a fallible call and then throws the error away is doing two:
> the transformation, and the decision to drop the failures.

**Avoid:**

```rust
directories
    .filter_map(|directory| fs::read_to_string(directory.join("Cargo.toml")).ok())
```

**Prefer:**

```rust
directories
    .map(|directory| directory.join("Cargo.toml"))
    .map(fs::read_to_string)
    .filter_map(Result::ok)
```

## Why restrict this?

This is a stylistic preference, not a correctness issue. The two forms
build the same iterator, with the same laziness and the same
short-circuiting, and `filter_map(|x| f(x).ok())` is ordinary idiomatic
Rust that a great deal of published code uses.

The preference is that the split form names the discard. In the folded
form `.ok()` is a syllable at the end of a closure, easy to write
without deciding anything and easy to read past; a reader asking "where
do the unreadable files go?" has to find it inside an expression that
is mostly about something else. As its own stage the discard is a line
of its own, and adding an `inspect` to log the errors, or swapping in
`filter_map(Result::ok)` for a `map(Result::unwrap_or_default)`, is an
edit to that line rather than surgery inside a closure.

The split also tends to make the transform point-free, which is why the
example above ends at `fs::read_to_string` rather than a closure
wrapping it: once `.ok()` is gone there is nothing left for the closure
to do. That reduction is `clippy::redundant_closure`'s to enforce, not
this rule's.

## What to lint

Flag a call to `Iterator::filter_map` whose closure body is exactly
`<expr>.ok()`, where `.ok()` resolves to `Result::ok`.

Both branches of the suggestion end at `filter_map(Result::ok)`, which
is the shape the rule is for. What differs is whether there is a
transformation to lift out of the way first.

**Where `<expr>` is more than the closure's parameter,** split the
adapter, preserving the closure verbatim minus the `.ok()`:

```rust
.filter_map(|binding| <expr>.ok())
// becomes
.map(|binding| <expr>).filter_map(Result::ok)
```

**Where `<expr>` is the parameter and nothing else,** there is no
transformation to split off, so drop the closure rather than splitting
it:

```rust
.filter_map(|result| result.ok())
// becomes
.filter_map(Result::ok)
```

Splitting that one would produce `map(|result| result)`, an identity
stage, which is why it is a separate branch rather than the same
rewrite applied blindly.

Either rewrite is mechanical and type-preserving, so both can be
`MachineApplicable`: `filter_map` and `map` are equally lazy, the item
type after the pair is what it was before, and the closure moves
unchanged or disappears.

### What Clippy already says

Measured on Clippy 1.94, a default `cargo clippy` run says nothing
about either branch.

`clippy::redundant_closure_for_method_calls` makes the same suggestion
as the second branch, but it is `pedantic` and therefore
allow-by-default, so a project sees it only after opting in. A project
that has will get both diagnostics on that line. Nothing in Clippy
covers the first branch at any level, which is the branch this rule
exists for.

### Exemptions

Do *not* flag:

- A closure whose body is anything but `<expr>.ok()` — a block with
  statements, a `?`, a `match`. Only the single-expression form has a
  mechanical split.
- `.ok()` that does not resolve to `Result::ok`. Other types define an
  inherent `ok`, and a method with the right name is not the method.
- A closure produced by a macro expansion, where the suggestion would
  be written into the macro body.
- A `filter_map` that is not `Iterator::filter_map` — `Option` and
  `Result` have their own.

### Why the trigger is `Result::ok` and not a family

Not because a closure returning `Option` by some other route is a
different anti-pattern. It is because `Result::ok` is the only tail
there is to strip.

`filter_map` wants an `Option`, so a body that already produces one has
nothing separable at its end: the only "split" available to it is
`map(..).flatten()`, which is a restatement of `filter_map` itself and
applies to every call of it ever written. A widened trigger does not
catch more of this anti-pattern; it stops describing one.

`Result::ok` is `fn ok(self) -> Option<T>`, and the rewrite rests on
both halves of that signature. Each is worth stating, because a future
widening would have to re-establish it:

- **Its only parameter is the receiver.** So the path `Result::ok` is
  already a function of one argument, which is the shape `filter_map`
  wants: the new adapter names it and needs no closure at all. A tail
  taking arguments besides the receiver would have to be re-wrapped in
  one, and the split would buy nothing.
- **That parameter is taken by value.** So the lifted tail consumes
  the item rather than borrowing from it, which is what makes the
  suggestion compile. It is not a general property of tails: see the
  `E0515` below, where a lifted `get` borrows from a value the adapter
  owns and drops.

So the narrowness is structural rather than a judgement about which
cases deserve flagging, and the case that looks like it needs an
exemption on type grounds — `filter_map(|x| x.lookup())` for a
`lookup` returning `Option` — needs none. It never matches, because
`lookup` is not `Result::ok`.

## Interaction with sibling rules

`perfectionist::overly_long_method_chain`
([`src/rules/overly_long_method_chain.rs`](../src/rules/overly_long_method_chain.rs))
**disagrees with this rule, and the disagreement is real.** Splitting
one adapter into two adds a distinct call to the chain, so code this
rule rewrites is code that one pushes closer to its limit. Measured on
the motivating case, the walk went from under the default limit to six
distinct calls against a `max_calls` of 5, and the remedy the chain
rule then offers — bind a stage to a `let` — undoes the shape this rule
asked for.

Both ship inactive, so nothing fires unless a project asks for it. Pick
one direction and do not enable both.

`clippy::manual_filter_map` pushes the other way, rewriting
`.filter(..).map(..)` into a single `filter_map`. It matches a
different shape, so the two do not fight over the same code, but a
project that wants adapters kept separate should know that clippy's
`complexity` group argues for folding them.

The name is deliberately *not* a variation on `manual_filter_map`. Per
[`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md), a
Clippy name may be mirrored only where the rule refines that lint; this
one contradicts it, so borrowing the name would claim a relationship it
does not have.

## Two neighbouring transformations that are *not* this rule

The review that produced this file proposed three changes at once. The
other two are recorded here so they are not re-proposed as sub-checks.

**Making the closure point-free** is `clippy::redundant_closure`, which
already covers it — `style`, and warn-by-default, so a project gets it
without opting in. (The near neighbour that fires on a *method* call
rather than a function call, `redundant_closure_for_method_calls`, is
`pedantic`; the two are not interchangeable, and the difference decides
the second branch above.) It is also a consequence rather than a
choice: the closure only becomes reducible once `.ok()` has moved out,
so it follows this rule rather than standing beside it.

**Lifting any nested call out of an adapter closure** — the general
form, of which the `.ok()` case is one instance — should not become a
rule. It has no canonical destination to lift into, so there is nothing
mechanical to suggest; and its advice does not always compile. The same
review proposed it for a `find` predicate:

```rust
// suggested
.filter_map(|manifest| manifest.get("workspace"))
.find(|workspace| workspace.is_table())
```

```
error[E0515]: cannot return value referencing function parameter `manifest`
```

`filter_map` hands the closure an owned value that dies at the end of
the step, so nothing may borrow out of it. A lint proposing that split
would have to know when the lifted sub-expression borrows from the
binding, which is the check that makes the general form hard and the
`.ok()` form easy: `Result::ok` takes `self`, so nothing is borrowed.

## Configuration

None. There is one trigger, and which of its two rewrites applies is
decided by the code rather than by taste. Whether a project wants the
rule at all is the `[perfectionist]` `enable` decision, not a knob.

## Implementation notes

A `LateLintPass`, because the trigger needs types: the receiver has to
be an `Iterator` and `.ok()` has to be `Result::ok` rather than an
inherent method of the same name.

- `check_expr` on `ExprKind::MethodCall` with the segment named
  `filter_map`, gated by `clippy_utils::is_trait_method(cx, expr,
  sym::Iterator)`.
- The single argument is an `ExprKind::Closure`; take its body and
  require `ExprKind::MethodCall` named `ok` whose receiver's type is
  the `Result` diagnostic item. `clippy_utils::ty::is_type_diagnostic_item`
  answers the receiver; resolving the method's `DefId` against
  `Result::ok` is the stricter form and is what the exemption above
  asks for.
- The suggestion needs two spans: the `filter_map` segment, and the
  `.ok()` call's span from the end of its receiver. `Sugg::hir` over
  the receiver gives the inner expression's text.
- Suppress proc-macro-synthesised nodes per
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).
  The diagnostic span is the method segment, which a derive can give a
  user-source span, so the guard is needed and a
  `ui/<rule>_proc_macro.rs` fixture should prove it.

### Difficulty

**Easy.** One expression, one closure, no cross-item reasoning, and a
suggestion that moves text rather than composing it. The only judgement
is the `Result::ok` resolution, which `clippy_utils` answers directly.

## Default state

Inactive by default. Enable in `[perfectionist].enable`.

Two independent reasons, either sufficient. It contradicts
`perfectionist::overly_long_method_chain`, and the catalogue's
convention for a contradicting pair is that a project picks one.
And the shape it flags is idiomatic: `filter_map(|x| f(x).ok())`
appears throughout published Rust, so a rule firing on it by default
would be arguing with most of its audience on first run.
