# `cloned_owned_argument`

**Source:** project convention, arising from the contingency in
[`cloned-borrowed-parameter.md`](./cloned-borrowed-parameter.md)'s
call-site proof. A parameter taken by value that the body never
consumes, where a caller clones to feed it.

## Statement

A signature that takes `T` by value asks each caller for ownership.
A caller holding a borrow can only answer with a clone. When the body
never consumes the value, that clone buys nothing: the callee reads
through it and drops it.

```rust
// `render` never consumes `key` — it reads a field and returns.
fn render(key: TaskKey) -> String {
    format!("{}:{}", key.name, key.path.display())
}

fn report(keys: &[TaskKey]) -> Vec<String> {
    keys.iter().map(|k| render(k.clone())).collect()   // a clone each
}
```

The complaint is not that the signature is wrong in the abstract. It
is that somebody is paying for it, and the payment is visible in the
crate.

## Why restrict this?

This is a stylistic preference rather than a correctness issue: the
program is right either way, and a by-value signature is often the
better interface. The objection is narrower — a caller is allocating
to satisfy a demand the body does not make.

It is also the failure mode
[`perfectionist::cloned_borrowed_parameter`](./cloned-borrowed-parameter.md)
leaves behind. That rule proves every production call site can give up
ownership *at the moment it fires*, then changes the signature — after
which its clause 1 no longer matches and it can never look at the
parameter again. A call site added later may hold a borrow, and
nothing re-checks the judgement. This rule is what re-checks it.

### Why a cloning call site is part of the trigger

Without it the rule is `clippy::needless_pass_by_value`, which fires
on a by-value parameter that is never consumed whether or not anyone
pays for it — measured on clippy 1.94, it fires on a function with
**no callers at all**, and on one whose only caller already owns the
value it passes. That is the right predicate for a lint about
interface style. It is the wrong one for a lint about a cost, because
most of its findings cost nothing.

Requiring the clone makes the finding evidential: there is an
allocation in this crate, at a named line, that the signature caused.

## What to lint

Flag a parameter `p` when all of:

1. Its written type is `T` by value, `T: Clone`, and `T` is not
   `Copy`. Skip `Rc` and `Arc`, as the sibling rules do: cloning one
   bumps a refcount.
2. No **hot** path through the body consumes `p` — every use borrows
   it, and any call it is passed to is one the summary says does not
   need it owned. Consumption on a cold path does not save the
   signature; see [Cold consumption](#cold-consumption).
3. At least one **production** call site passes a clone of a place it
   does not own — `f(x.clone())`, `f(x.to_owned())`, or an equivalent
   the summary recognises.
4. The parameter is not externally reachable, per
   [The visibility bound](./IMPLEMENTATION_CONVENTIONS.md#the-visibility-bound).

Clause 2 is the negation of the question
`perfectionist::cloned_borrowed_parameter` asks, and the two must
resolve it through one summary rather than each deciding for itself;
see
[One notion of "needs owned"](./IMPLEMENTATION_CONVENTIONS.md#one-notion-of-needs-owned-read-by-every-consumer).

### Why clause 4 is courtesy rather than soundness

The sibling needs the crate boundary because it cannot *prove* its
claim without every call site. This rule can: `T` → `&T` costs no
caller anything. A caller that owns its value passes `&x` instead of
`x`, which is free; a caller that holds a borrow passes it along and
saves a clone. There is no third kind.

So clause 4 is here for a different reason — changing a `pub`
signature is a breaking change for downstream crates, and this rule's
payoff is an allocation the author may not consider worth a major
version. A crate-local parameter has no such cost.

Note that this also makes clause 3 **existential** where the sibling's
call-site clause is universal. Missing a call site costs a finding,
never a wrong one.

### Cold consumption

A body that consumes `p` only through an error arm or a panic path
still leaves every caller cloning on the common path to serve the rare
one. That is the same asymmetry
[the sibling's cold-path filter](./cloned-borrowed-parameter.md#why-clause-2-excludes-cold-paths)
rejects, read in the other direction, and it uses the same signals.

It is also the case `clippy::needless_pass_by_value` cannot reach —
measured silent, because the body does consume — which makes it this
rule's most valuable finding rather than an edge case.

The fix there is not `&T` alone: the cold arm still needs an owned
value, so it clones *inside* the arm. That moves one clone from every
call to the calls that take the arm.

## Examples

**Avoid:** the callers pay for ownership the body never takes.

```rust
fn render(key: TaskKey) -> String { … }        // `key` only read

fn report(keys: &[TaskKey]) -> Vec<String> {
    keys.iter().map(|k| render(k.clone())).collect()
}
```

**Prefer:**

```rust
fn render(key: &TaskKey) -> String { … }

fn report(keys: &[TaskKey]) -> Vec<String> {
    keys.iter().map(render).collect()
}
```

**Avoid:** consumed, but only on the cold arm.

```rust
fn record(key: TaskKey, log: &mut Vec<TaskKey>) -> Result<()> {
    let written = write_entry(&key)?;
    if !written {
        log.push(key);                          // cold: the retry list
    }
    Ok(())
}
```

**Prefer:** the clone moves to the arm that needs it.

```rust
fn record(key: &TaskKey, log: &mut Vec<TaskKey>) -> Result<()> {
    let written = write_entry(key)?;
    if !written {
        log.push(key.clone());
    }
    Ok(())
}
```

**Not flagged:** every caller owns what it passes, so nobody is
paying. `clippy::needless_pass_by_value` still fires here, and that is
the difference between the two lints.

```rust
fn consume_name(name: String) -> usize { name.len() }

fn main() {
    consume_name(read_name());
}
```

## Suggested fix

Change the parameter to `&T` and drop the clone at the call sites the
diagnostic names. The lint reports **once per parameter**, spanned at
the parameter, listing the cloning call sites as evidence — the name
says what the trigger sees, not where it points, and a finding per
call site would report one signature *n* times.

The edit is mechanical where clause 2 holds on every path, and the
suggestion may be `MachineApplicable` there. Under
[Cold consumption](#cold-consumption) it is not: the cold arm needs a
clone inserted, which is a judgement about where the cost belongs, so
the suggestion is a note.

## Configuration

```toml
["perfectionist::cloned_owned_argument"]
# Whether test code / a build script is exempt. Both default to
# `true`, and each governs two things: whether such a body is flagged,
# and whether a call site there can satisfy clause 3.
exempt_tests = true
exempt_build_scripts = true
```

The same two knobs as the sibling rules, so a project that has taken a
position on one has taken it on all three. A test that clones to call
a production function is weak evidence — the test is not the workload
— which is why it does not satisfy clause 3 by default.

## Implementation notes

`LateLintPass::check_fn` for clauses 1 and 2, reading
`crate::ownership_summary` for any call `p` is passed to, plus a
crate-wide walk for clause 3. Clause 2 is the summary's own answer
about this parameter, so the rule contributes to the summary as much
as it reads it.

### Difficulty

**Medium.** Clause 2 is the summary's question and comes for free once
that module exists. Clause 3 is a call-graph walk collecting cloning
arguments, which the sibling's clause 4 already needs in the other
polarity. The work specific to this rule is recognising "a clone of a
place the caller does not own", which is `clippy_utils`'
clone-detection plus a local ownership check.

Without the summary it is **Easy** and narrower: clause 2 restricted
to bodies that pass `p` to nothing, which still catches the motivating
shape.

### Evidence

None yet, and the shape is constructed rather than observed. The
honest negative result is worth recording: the sibling's flagship case
does **not** become a finding here. After pnpm's change,
`record_passed` takes `TaskKey` by value and consumes it —
`writer.completed.insert(key)` on the success path — so clause 2 fails
and this rule is silent. The two test call sites that gained a
`.clone()` are exempt under `exempt_tests` and would not satisfy
clause 3 either.

That is the rule working as intended rather than a gap, but it means
the drift scenario this rule exists for has not been observed in the
wild, only argued.

### Which rule this reverses

Three rules share the borrowed-versus-owned axis, and this one is not
equally opposite to the other two:

| Rule                           | Edit       | Body predicate                                             | Call-site clause               |
| ------------------------------ | ---------- | ---------------------------------------------------------- | ------------------------------ |
| `needless_borrowed_parameters` | `&T` → `T` | converts unconditionally, used once, recognised pointee    | none                           |
| `cloned_borrowed_parameter`    | `&T` → `T` | **∃** hot path needs owned                                 | **∀** production sites can give |
| `cloned_owned_argument`        | `T` → `&T` | **¬∃** hot path needs owned                                | **∃** production site pays     |

Against `needless_borrowed_parameters` the reversal is directional
only. That rule has no call-site clause to dualise — its
unconditionality gate is what lets it stay callee-local — and its
other gates have no counterpart here.

Against `cloned_borrowed_parameter` it is a dual in form: the body
predicate is negated and the call-site quantifier flips. The flip is
not decoration. That rule's edit can *harm* a caller, since a borrower
forced to clone pays what the callee used to, so it needs every call
site to be safe. This rule's edit harms nobody — an owning caller
passes `&x` for free — so it needs only evidence that the change is
worth making. Proof against ∀, evidence against ∃.

The same asymmetry explains why neither of the others' findings
becomes one of this rule's by accident. Both fire only where the body
needs ownership, which is exactly what clause 2 requires it not to.
A signature they moved to `T` comes back only if the body stops
consuming, or only consumes on a
[cold path](#cold-consumption).

Where the body genuinely consumes on a hot path and a caller still
clones, no rule fires and none should: taking `&T` would move the
clone into the callee and pay it on every call instead. The caller's
clone is not waste there, it is the cost of the value.

### The pair must not ping-pong

Applying either rule's fix must not produce a finding for the other.
That is a correctness criterion for the pair, not a nicety: a loop
between two lints is worse than either lint being absent, because a
reader following both is told to undo the change they just made.

**The sibling fires, its fix lands — does this rule?** No, and by
construction rather than by luck. Its clause 2 is "some hot path needs
an owned `T`", and its fix removes the copy so that path consumes the
parameter itself. "The body consumes `p` on a hot path" is exactly the
negation of clause 2 here. One predicate decides both, which is what
[one summary](./IMPLEMENTATION_CONVENTIONS.md#one-notion-of-needs-owned-read-by-every-consumer)
buys: two rules computing it separately could disagree, and a
disagreement between duals is a loop.

**This rule fires, its fix lands — does the sibling?** Not in the
plain case. Clause 2 says no hot path consumed the parameter, so after
the fix no hot path needs it owned and the sibling's clause 2 has no
witness.

The [cold case](#cold-consumption) is the one that would loop, and it
is the real reason the sibling's cold-path filter is load-bearing. The
fix there leaves `p.clone()` inside the cold arm — a copy of a
borrowed parameter moved into a collection, which is the sibling's
clause 2.1 read literally. Two things stop it:

- **The filter**, because that copy is on a cold path and the
  sibling's clause 2 requires a witness that is not exclusively cold.
- **Its clause 4**, because the caller whose clone triggered this rule
  now passes a borrow, so not every production call site owns what it
  passes.

Only the first survives indefinitely. That caller can stop cloning, or
be deleted, and then clause 4 no longer objects — leaving the filter
as the barrier that has to hold. So turning the filter off would not
merely restore some pessimising findings; it would make the two rules
undo each other's fixes, which is the strongest reason it is
[not a knob](./cloned-borrowed-parameter.md#configuration).

`perfectionist::needless_borrowed_parameters` is not part of the loop.
After the plain fix the body performs no conversion for its gates to
inspect, and after the cold fix the parameter is used twice — the cold
clone and the hot borrow — where that rule requires exactly one use.
The single window is a body whose *only* use is a cold clone, with a
pointee its pair list recognises, written in a `let … else` block,
which its
[unconditionality defect](./needless-borrowed-parameters.md#status)
fires on today. Fixing that defect closes the window; nothing here
needs to work around it.

## Default state

Active by default. Unlike the sibling, this rule's failure direction
is silence: clause 3 is existential, so a call site the walk misses
costs a finding; clause 2 is the summary's own answer, and an
unresolved call leaves the parameter unflagged. Nothing it reports can
make a caller worse, because `T` → `&T` costs no caller anything.

What it can do is churn a signature for one allocation, which is why
clause 3 requires evidence and clause 4 keeps it inside the crate.

## Interaction with clippy and sibling rules

- **`clippy::needless_pass_by_value`** (`pedantic`, allow by default)
  is this rule without clauses 3 and 4 and without the cold case. A
  genuine refinement, which under
  [Mirror the Clippy name only for a genuine refinement](./IMPLEMENTATION_CONVENTIONS.md#mirror-the-clippy-name-only-for-a-genuine-refinement)
  is the condition for borrowing its name — this rule does not, because
  its trigger is the *argument* rather than the parameter and the name
  should say which. A project running the clippy lint gets a superset
  and needs neither.
- **`perfectionist::cloned_borrowed_parameter`** is the one this rule
  is the dual of, in a stricter sense than "points the other way".
  The two cannot fire on one parameter, since one needs `&T` and the
  other `T`, but they can disagree across a call chain if they answer
  the shared question separately; the summary is what prevents that.
- **`perfectionist::needless_borrowed_parameters`** is the third rule
  on this axis. This rule reverses its *direction* and nothing else —
  see [Which rule this reverses](#which-rule-this-reverses).
- **`clippy::redundant_clone`** (`nursery`) flags a clone whose result
  is dropped without further use. It sees one call site and not the
  signature that caused it, so it misses the case where the clone is
  genuinely consumed — by being moved into the callee.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
