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

Both blocks are reduced to the call that changed; the real call site
chains two further setters and spells its receiver differently.

Every pair reads the same way. Arguments are the pair most likely to be
met in the wild, and a closure is how the fold is most likely to be
written:

**Avoid:**

```rust
flags.iter().fold(Command::new("ls"), |c, f| c.with_arg(f))
```

**Prefer:**

```rust
Command::new("ls").with_args(flags)
```

The plural is not a convenience wrapper the caller could take or leave.
Upstream defines `with_args` as
`args.into_iter().fold(self, Self::with_arg)`, so the hand-rolled
version is not merely equivalent to the plural — it is the plural's
body, inlined at the call site, one level up from where the library
already wrote it.

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
2. The folder — `fold`'s second argument — *resolves* to a singular
   `CommandExtra` setter. Resolution is the trigger, not spelling: one
   method has many spellings, and all of them fold. The ones to expect,
   which are not an exhaustive set:
   - a **path** — `CommandExtra::without_env`, `Command::without_env`,
     `<Command as CommandExtra>::without_env`, `Self::without_env`, or
     any of those reached through a renamed import;
   - a **forwarding closure** — one whose body is a single call to that
     setter, passing the closure's parameters in order, unmodified, and
     doing nothing else. A tuple parameter destructured in the pattern
     still forwards, so long as its bindings are passed in the order
     they were bound; the `with_env` row below needs that. The call may
     be written as a method call
     (`|acc, item| acc.without_env(item)`) or as an associated-function
     call on any of the paths above
     (`|acc, item| CommandExtra::without_env(acc, item)`);
   - a **path bound to a local** — `let f = CommandExtra::without_env;`
     and then `.fold(command, f)`. It compiles and folds exactly like
     the bare path, but the folder resolves to the local rather than to
     the setter, so catching it means following the binding's
     initialiser. A first implementation may skip that, as a known gap
     rather than because the predicate excludes it.
3. That setter has a plural counterpart in the table below.
4. The `fold` receiver is a **simple iterator expression**: a place
   expression — `VARS`, `self.vars`, `cfg.env_names` — followed by at
   most one argument-less method call. The method is not drawn from a
   fixed list: `list.iter()`, `list.into_iter()`, `self.vars.iter()`
   and a bare iterator binding all qualify, as would a method of your
   own.

Condition 4 is a value gate rather than a correctness one. The rewrite
stays valid for any receiver, because the receiver only moves; it stops
being an *improvement* once what moves is long or carries logic of its
own. These are equivalent, but *must not* fire:

```text
    list.iter().map(mapper).fold(B, f)   ->  B.plural(list.iter().map(mapper))
    list.iter().filter(pred).fold(B, f)  ->  B.plural(list.iter().filter(pred))
    list.into_iter().rev().fold(B, f)    ->  B.plural(list.into_iter().rev())
    COMPLEX_EXPRESSION.fold(B, f)        ->  B.plural(COMPLEX_EXPRESSION)
```

Each has relocated a chain into argument position rather than removed
one — the opposite of what this rule is for. The condition bars them
two ways, and neither is a list of adapter names that would need
extending as the iterator API grows:

- **No arguments**, because an argument is where the logic hides. The
  reader would still have to work out what `mapper` yields.
- **At most one call**, because a second is more text moving. `rev`
  carries no logic and is still barred: `list.into_iter().rev()` is
  simply longer than the rule is willing to relocate.

Unlike condition 2, this one is deliberately syntactic: what it
measures is how much text the suggestion relocates, which is a property
of the written form rather than of what anything resolves to. That
leaves a known gap — `Vec::into_iter(list).fold(B, f)` is the same
shape written as an associated-function call, and does not match.

| singular      | plural         | example closure                                  |
|---------------|----------------|--------------------------------------------------|
| `with_arg`    | `with_args`    | `\|acc, item\| acc.with_arg(item)`               |
| `without_env` | `without_envs` | `\|acc, item\| acc.without_env(item)`            |
| `with_env`    | `with_envs`    | `\|acc, (key, value)\| acc.with_env(key, value)` |

The `with_env` row is the awkward one and also the most valuable. Its
item is a tuple, so the closure destructures, and no path spelling
works: `with_env` takes three arguments where `fold` supplies two, so
the arity does not match and the compiler rejects every one of them. A
closure is therefore mandatory for that pair, which makes it the shape
most likely to be written by hand and left alone.

### Exemptions

The rule fires only where the rewrite *deletes* something — the fold,
the closure, the `.iter()`. Where the rewrite would merely move
something somewhere else, it must stay quiet. Condition 4 above is that
principle applied to the receiver; the first exemption below is the
same principle applied to the folder.

- **A closure the rewrite would not remove.** The point of the plural
  is that the closure disappears: `|c, a| c.with_arg(a)` becomes
  nothing at all, because `with_args` *is* that fold. A closure that
  computes on the way survives instead —
  `|c, a| c.with_arg(format!("--{a}"))` rewrites to
  `with_args(items.iter().map(|a| format!("--{a}")))`, which is the
  same lambda moved one call to the left, plus a `map` that was not
  there before. So the closure must forward its parameters untouched,
  and anything else — an extra statement, a `?`, a conditional,
  arguments passed out of order, an item used twice — must not fire.

- **A singular with no plural.** `with_no_env`, `with_stdin`,
  `with_stdout` and `with_stderr` have no plural counterpart — each
  sets one thing that a later call replaces rather than extends — so a
  fold over them is a different mistake and outside this rule.
- **A plural the resolved `command-extra` does not have.** The pairs
  arrived over several releases — `without_envs` is 1.2.0 and later —
  so a crate can have the singular without its plural, which is the
  very reason its author wrote the fold. Naming a plural that release
  does not have suggests code that will not compile, so the table
  above is checked against the trait rather than taken on trust.
- **An accumulator that is not a `Command`.** Resolving the setter
  already implies this, so it costs the trigger nothing; it is spelled
  out because an implementation that matches setter *names* instead of
  resolving them would lose it silently.
- **Macro-synthesised calls**, per
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).

## Configuration

None. The suggestion is sound only because upstream defines each plural
as exactly the fold of its singular; a pair named for some other builder
could not promise that, so the set is fixed rather than tunable.

## Implementation notes

A `LateLintPass` over expressions, matching a method call that
resolves to `core::iter::Iterator::fold`. The parts that need care, in
rising order of difficulty:

**Resolving the accumulator.** `cx.typeck_results()` on the
initial-value argument, compared against `std::process::Command` by
`DefId`. Cheap, and it is what stops the rule firing on unrelated
folds.

**Matching the folder.** Resolve the folder's callee to a `DefId` and
compare that against the singular's method. Comparing the resolved item
rather than the written path is what makes the spellings listed above
fall out for free: `Command::without_env`,
`<Command as CommandExtra>::without_env` and a renamed import all
resolve to the one method, so none of them needs a case of its own. For
a bare path that comparison is the whole check.

A closure needs its body walked first: one expression, a call, whose
callee is that method and whose arguments are exactly the closure's
parameters — or, where one of them is a destructured tuple, its
bindings — in order. The method-call and associated-function spellings
differ only in where HIR puts the receiver — `ExprKind::MethodCall`
keeps it out of the argument list, `ExprKind::Call` has it as the first
argument — so normalise to receiver-then-arguments and compare once
rather than matching each syntax separately. Anything else fails the
match. This is the same shape of check
`clippy::redundant_closure_for_method_calls` performs, and its
implementation is worth reading first.

**Checking the plural exists.** `trait_of_assoc` on the folder's
`DefId` reaches the `CommandExtra` that compilation loaded, whose
associated items say which plurals that release has. Cheap — the
folder has already resolved into the trait — and it carries no version
table, so a plural dropped or renamed later is covered by the same
question.

**Building the suggestion.** The fix is not a token swap. The
iterator being folded is the *receiver* of `.fold(...)`, and it has to
become the argument of the plural:

```text
    list.iter().fold(B, f)       ->  B.plural(list.iter())
    list.into_iter().fold(B, f)  ->  B.plural(list)
```

so the receiver and `B` both move. Condition 4 keeps the receiver
short, so those are the whole shape rather than instances of a wider
one; `B` is unconstrained and may be a multi-line expression.

The rows differ in whether the call survives. Condition 4 allows at
most one, so the whole story is four cases:

- **`list`** (no call) — nothing to erase. `B.plural(list)`.
- **`list.into_iter()`** — erased when the call resolves to
  `IntoIterator::into_iter`, which is the very function `B.plural`
  calls, so the two agree by construction. Compare the `DefId`, not
  the name: an *inherent* `into_iter` shadows the trait in method
  resolution and need not agree with it.
- **`list.iter()`** — erased when both hold:
  - The call resolves into `core` / `std` / `alloc`. There is no
    `Iterator::iter` to compare against — every `iter` is an inherent
    method of its own type, and `Vec`'s is `<[T]>::iter` reached
    through `Deref` — so the only guarantee available is std's
    convention that `&C: IntoIterator` agrees with `C::iter()`. A
    local `iter` promises nothing.
  - `list` is already a reference. On an owned collection the erasure
    moves what the fold merely borrowed, and the code around it stops
    compiling.

  `UI_HARNESS_VARS` is a `&'static` slice whose `iter` is std's, which
  is why the fixed call site reads `without_envs(UI_HARNESS_VARS)`.
- **any other method** — survives verbatim.
  `B.plural(list.some_method())`.

Erasing past those checks fails silently rather than loudly: a local
`iter` or `into_iter` that disagrees with the trait still compiles, and
reorders or drops arguments. Keeping the call is always safe, and a
first implementation may do that throughout.

A conservative first implementation: **path folders only, and only
the pairs whose item is a single value.** That covers what this
repository actually wrote, needs no closure-body analysis, and leaves
the `with_env` tuple case — the hardest and the most valuable — for a
follow-up that can lean on the closure matcher once it exists.

### Evaluation order

The suggestion trades the receiver and the initial value, so it trades
the order they run in:

- `A.fold(B, f)` evaluates `A`, then `B`.
- `B.plural(A)` evaluates `B`, then `A`.

It takes *both* sides to make that observable. A mutating receiver on
its own is not enough — against a plain binding the two forms cannot be
told apart:

```rust
// `drain_all` takes only `&mut self` and empties the queue, so
// condition 4 admits it. `cmd` is a binding, so evaluating it does
// nothing. Same command, same queue, either way round.
args.drain_all().fold(cmd, CommandExtra::with_arg)
cmd.with_args(args.drain_all())
```

The difference needs an initial value that observes what the receiver
changes:

```rust
args.drain_all()
    .fold(Command::new(format!("ls{}", args.0.len())), with_arg)
// drains first, so `len()` sees 0    ->  program "ls0"

Command::new(format!("ls{}", args.0.len())).with_args(args.drain_all())
// `len()` first, sees 2, then drains ->  program "ls2"
```

Trigger and autofix part ways:

- **Fire** on every receiver condition 4 admits. The diagnostic is
  right whatever the order.
- **Machine-applicable** where the receiver's call resolves into `core`
  or `std` — a `DefId` origin check, not a roster and not effect
  analysis. It passes every receiver reached through `iter` /
  `into_iter` on a std collection, this repository's own call site
  included.
- **Advice only** otherwise, the same answer
  `mutating_command_builder` gives where the receiver is a binding.

That gate is over-conservative on purpose, and the cost is worth
naming rather than discovering: it declines a fix for the first example
above, which is provably safe. One side is cheap to classify and two
are not, and mutation reordered is the kind of wrong that does not
announce itself. A later implementation may widen it to *either* side
pure, counting an initial value as pure when it is a place expression,
a literal, or a `core` / `std` call over those.

Fixtures worth writing:

- `list.iter()`, `list` a slice — fires; autofix erases to
  `plural(list)`.
- `list.iter()`, `list` an owned `Vec` still used afterwards — fires;
  autofix keeps `plural(list.iter())`, since erasing would move what
  the fold only borrowed.
- `list.into_iter()` — fires; autofix erases to `plural(list)`.
- `weird.iter()`, an inherent `iter` of the linted crate disagreeing
  with its own `IntoIterator` impl — fires; autofix keeps
  `plural(weird.iter())`. Erasing would silently reorder.
- `shadow.into_iter()`, an inherent `into_iter` shadowing the trait —
  fires; autofix keeps `plural(shadow.into_iter())`.
- `queue.drain_all()` with a plain binding as the initial value —
  fires; advice only. Pins the over-conservatism deliberately: the
  rewrite is safe here and the gate declines it anyway.
- `queue.drain_all()` with an initial value that reads the queue —
  fires; advice only, and this one stays advice under any gate.
- `list.into_iter().rev()` — silent; two calls, per condition 4.
- `list.iter().map(mapper)` — silent; the call takes an argument.
- `Vec::into_iter(list)` — silent; the known syntactic gap, not a
  regression.

### Difficulty

**Medium.** The trigger is local to one expression, so this is nowhere
near the whole-crate reasoning
[`manual-lazy-init.md`](./manual-lazy-init.md) needs. What raises it
above easy is that the folder match and the suggestion are structural
rather than nominal: recognising a forwarding closure means
pattern-matching a body rather than comparing a name, and the suggestion
rearranges two sub-expressions instead of renaming one. The conservative
subset above is genuinely easy, and it is a real subset rather than a
token gesture.

### Default state

Active by default.

No dependency gate is needed, unlike its sibling: the trigger names
`CommandExtra` methods, so it can only fire in code that already
calls them.

## Interaction with sibling rules

`perfectionist::mutating_command_builder`
([`src/rules/mutating_command_builder.rs`](../src/rules/mutating_command_builder.rs))
flags std's `&mut self` setters where the by-value form exists. The
two rules are orthogonal in the direction that matters: this rule
fires on code that one considers already correct. The fold above uses
`CommandExtra` throughout and contains no std setter at all, so the
sibling has nothing to say about it, which is the argument for these
being two rules rather than sub-checks of one.

They do not overlap; they compose. Given a fold over the *std* setter:

```rust
items.iter().fold(command, |mut c, a| { c.arg(a); c })
```

this rule is silent. The folder resolves to `Command::arg` rather than
a `CommandExtra` setter, and the body is a block rather than a
forwarding call, so condition 2 and the first exemption exclude it
twice over. The sibling is the rule that speaks here, on the `c.arg(a)`
inside. Once its advice is taken and the closure collapses to
`|c, a| c.with_arg(a)`, this rule fires on the result and suggests the
plural.

So neither rule stands down for the other: doing so would leave this
shape flagged by nobody.

`perfectionist::overly_long_method_chain`
([`src/rules/overly_long_method_chain.rs`](../src/rules/overly_long_method_chain.rs))
caps the calls on a chain's spine, and this rule shortens a spine
rather than lengthening it — replacing `.iter().fold(...)` with one
plural call removes two. The two therefore pull the same way, unlike
`perfectionist::pipe_style`, which pulls against it.
