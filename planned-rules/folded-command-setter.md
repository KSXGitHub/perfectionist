# `folded_command_setter`

**Sources:** this repository's own history. `utils/src/dylint.rs`
folded `CommandExtra::without_env` over a list of variable names
because the plural did not exist yet; `command-extra` 1.2.0 added
`without_envs`, and
<https://github.com/KSXGitHub/perfectionist/pull/450> replaced the
fold with it. The fold was written by an author who had no way to know
the plural was coming, which is the situation this rule exists to
catch for everyone after them.

## Statement

`CommandExtra`'s setters come in singular and plural pairs. Folding
the singular over an iterator re-implements the plural by hand:

**Avoid:**

```rust
UI_HARNESS_VARS
    .iter()
    .fold(Command::new("cargo"), CommandExtra::without_env)
```

**Prefer:**

```rust
Command::new("cargo").without_envs(UI_HARNESS_VARS)
```

The plural is not a convenience wrapper the caller could take or
leave. Upstream implements it as exactly this fold, so the hand-rolled
version duplicates library code that already exists, one level up.

## Why restrict this?

This is a stylistic preference, not a correctness issue. The fold is
correct and produces an identical `Command`.

The preference is that a fold over a builder setter makes the reader
reconstruct an operation the API already names. `fold` is a
general-purpose combinator, so encountering one obliges the reader to
work out what is being accumulated and in what order before they can
see that the answer is "these variables are removed". `without_envs`
says that outright, and the receiver stops being an accumulator
threaded through a closure.

There is a second, smaller reason: the fold form has more places to
get wrong. A fold whose closure swaps its accumulator and item, or
returns the wrong one of the two, compiles in some shapes and
silently builds the wrong command. The plural has no such surface.

## What to lint

A call to `Iterator::fold` where all of the following hold:

1. The initial-value argument's type is `std::process::Command`.
2. The folding argument is a singular `CommandExtra` setter, in one of
   two spellings:
   - a **path** — `CommandExtra::without_env`, `Self::without_env`;
   - a **forwarding closure** — `|acc, item| acc.without_env(item)`,
     which passes both of its parameters, in order, unmodified, to a
     single method call and does nothing else.
3. That setter has a plural counterpart in the `pairs` table below.

| singular      | plural         | closure shape                                    |
|---------------|----------------|--------------------------------------------------|
| `with_arg`    | `with_args`    | `\|acc, item\| acc.with_arg(item)`               |
| `without_env` | `without_envs` | `\|acc, item\| acc.without_env(item)`            |
| `with_env`    | `with_envs`    | `\|acc, (key, value)\| acc.with_env(key, value)` |

The `with_env` row is the awkward one and also the most valuable. Its
item is a tuple, so the closure destructures, and there is no path
spelling at all: `Self::with_env` takes three arguments where `fold`
supplies two, so the arity does not match and the compiler rejects it.
A closure is therefore mandatory for that pair, which makes it the
shape most likely to be written by hand and left alone.

### Exemptions

- **A folding closure that does anything else.** An extra statement, a
  `?`, a conditional, arguments passed out of order, an item used
  twice. The plural is only equivalent to a closure that forwards and
  nothing more; anything else must not fire.
- **A singular with no plural.** `with_no_env`, `with_stdin`,
  `with_stdout` and `with_stderr` take no per-item value and have no
  plural form, so a fold over them is a different mistake and outside
  this rule.
- **An accumulator that is not a `Command`.** The `pairs` table names
  `CommandExtra` methods, so this follows from the method resolving,
  but the type check is worth making explicit rather than inferring it
  from the name.
- **Macro-synthesised calls**, per
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).

## Configuration

```toml
["perfectionist::folded_command_setter"]
# The singular/plural pairs the lint recognises, as
# `[singular, plural]`. The defaults are `command-extra`'s complete
# set of paired setters as of 1.2.0. Extend this for a builder of
# your own that follows the same singular/plural convention; the
# trigger only requires that the accumulator's type owns both
# methods.
pairs = [
  ["with_arg", "with_args"],
  ["with_env", "with_envs"],
  ["without_env", "without_envs"],
]
```

## Implementation notes

A `LateLintPass` over expressions, matching a method call that
resolves to `core::iter::Iterator::fold`. Three parts need care, in
rising order of difficulty.

**Resolving the accumulator.** `cx.typeck_results()` on the
initial-value argument, compared against `std::process::Command` by
`DefId`. Cheap, and it is what stops the rule firing on unrelated
folds.

**Matching the folder.** The path spelling is a `DefId` comparison
against the singular's method. The closure spelling needs the body
walked: one expression, a method call, whose receiver is the closure's
first parameter and whose argument list is exactly the remaining
parameters in order. Anything else fails the match. This is the same
shape of check `clippy::redundant_closure_for_method_calls` performs,
and its implementation is worth reading first.

**Building the suggestion.** The fix is not a token swap. The
iterator being folded is the *receiver* of `.fold(...)`, and it has to
become the argument of the plural:

```text
    A.iter().fold(B, f)   ->   B.plural(A.iter())
```

so `A` and `B` both move, and either may be a multi-line expression.
Where `A` is `<slice>.iter()` the suggestion can often drop the
`.iter()` as well, because the plural takes `IntoIterator` and a
reference to a slice already satisfies it — that is what made the
fixed call site read `without_envs(UI_HARNESS_VARS)` rather than
`without_envs(UI_HARNESS_VARS.iter())`. Dropping it is an
improvement, not a requirement, and a first implementation may keep
the `.iter()`.

A conservative first implementation: **path folders only, and only
the pairs whose item is a single value.** That covers what this
repository actually wrote, needs no closure-body analysis, and leaves
the `with_env` tuple case — the hardest and the most valuable — for a
follow-up that can lean on the closure matcher once it exists.

### Difficulty

**Medium.** The trigger is local to one expression, so this is
nowhere near the whole-crate reasoning
[`manual-lazy-init.md`](./manual-lazy-init.md) needs. What raises it
above easy is that two of the three parts are structural rather than
nominal: recognising a forwarding closure means pattern-matching a
body rather than comparing a name, and the suggestion rearranges two
sub-expressions instead of renaming one. The conservative subset above
is genuinely easy, and it is a real subset rather than a token
gesture.

### Default state

Active by default.

No dependency gate is needed, unlike its sibling: the trigger names
`CommandExtra` methods, so it can only fire in code that already
calls them.

## Interaction with sibling rules

`mutating_command_builder`
([`mutating-command-builder.md`](./mutating-command-builder.md))
flags std's `&mut self` setters where the by-value form exists. The
two rules are orthogonal in the direction that matters: this rule
fires on code that one considers already correct. The fold above uses
`CommandExtra` throughout and contains no std setter at all, so the
sibling has nothing to say about it, which is the argument for these
being two rules rather than sub-checks of one.

They meet on a single shape:

```rust
items.iter().fold(command, |mut c, a| { c.arg(a); c })
```

a fold over the *std* setter. This rule's suggestion subsumes the
sibling's, so the sibling should stand down inside a fold this rule
already flags. See that file's own interaction section for the same
statement from the other side.

`perfectionist::overly_long_method_chain`
([`src/rules/overly_long_method_chain.rs`](../src/rules/overly_long_method_chain.rs))
caps the calls on a chain's spine, and this rule shortens a spine
rather than lengthening it — replacing `.iter().fold(...)` with one
plural call removes two. The two therefore pull the same way, unlike
`pipe_style`, which pulls against it.
