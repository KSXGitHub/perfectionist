# `cloned_borrowed_parameter`

**Source:** project convention, distilled from two pnpm changes —
[`pnpm/pnpm#15001`](https://github.com/pnpm/pnpm/pull/15001) and
[`pnpm/pnpm#15076`](https://github.com/pnpm/pnpm/pull/15076) — that the
implemented `perfectionist::needless_borrowed_parameters`
([`src/rules/needless_borrowed_parameters.rs`](../src/rules/needless_borrowed_parameters.rs))
cannot reach. This is that rule's sibling, not a loosening of it; see
[Why a sibling and not a config knob](#why-a-sibling-and-not-a-config-knob).

## Statement

A function takes a parameter by shared reference and clones it into
something that outlives the call, while every call site already owns
the value and drops it immediately afterwards. Every call pays for a
clone nobody needed: the caller built the value, lent it, the callee
cloned it, and the caller dropped the original.

Taking the parameter by value moves the caller's value straight in.

### Why this name

The proposal
([`KSXGitHub/perfectionist#471`](https://github.com/KSXGitHub/perfectionist/issues/471))
called this rule `copied_lent_parameter` and invited a better name.
That one has two problems. *Lent* is not the word Rust uses for the
callee's side of a shared reference — the language, the compiler's
diagnostics, and the sibling rule all say **borrowed**. And *copied*
reads as the `Copy` trait, which the trigger explicitly excludes:
[clause 1](#what-to-lint) requires `Clone` and **not** `Copy`.

`cloned_borrowed_parameter` fixes both while keeping the shape
[Naming a lint after the anti-pattern](./IMPLEMENTATION_CONVENTIONS.md#naming-a-lint-after-the-anti-pattern)
asks for: an adjective-plus-noun phrase naming the offending
construct rather than the remedy, claiming no more than the trigger
checks. `#[deny(perfectionist::cloned_borrowed_parameter)]` forbids a
borrowed parameter that gets cloned;
`#[allow(perfectionist::cloned_borrowed_parameter)]` permits one.

## Why restrict this?

This is a stylistic preference, not a correctness issue. Both
signatures compile and behave identically; the borrowed one just
allocates more.

Let *k* be the number of owned clones the callee needs.

- **Borrowed parameter:** the callee clones *k* times; the caller pays
  nothing.
- **Owned parameter:** the callee receives one owned value and clones
  *k − 1* times; a caller that owns the value pays nothing, a caller
  that holds a borrow pays one.

So by-value saves exactly one clone for an owning caller and is
neutral for a borrowing one, which is the same trade the sibling rule
already documents. Borrowing uses inside the callee cost nothing
either way, since a value you own can always be borrowed from, so a
body that mixes borrows with one escaping clone is exactly the shape
this rule is for.

### Why clause 4 is load-bearing

That trade holds only while the clone is unconditional. When the
callee clones on some paths and not others, a caller holding a borrow
pays for a clone the callee might never have made. In
[`parse_specifier`](#package-specifier-parsing), such a caller would
clone for *every* selector, including the purl ones the borrowed
signature never cloned.

This is why the sibling rule's unconditional gate is not conservatism
for its own sake: it is the precondition that lets that rule stay
callee-local and skip caller analysis entirely. This rule admits
conditional clones, so it has to buy the same guarantee elsewhere — by
proving that no caller holds a borrow in the first place. That proof
is [clause 4](#what-to-lint), and dropping it to make the rule cheaper
would leave a rule that is wrong about the very cases it exists to
catch.

## Motivating cases

Both were found by auditing pnpm against the sibling rule, and each
has a fix in the pull request cited with it.

### Package-specifier parsing

[`pnpm/pnpm#15001`](https://github.com/pnpm/pnpm/pull/15001), condensed:

```rust
fn parse_specifier(request: &AddRequest) -> Result<ParsedSpecifier> {
    let specifier = request.selector();          // borrowing use
    // …
    let Some(body) = purl::strip_scheme(specifier) else {
        return Ok(ParsedSpecifier::Node(request.clone()));   // clone escapes
    };
    parse_purl(&Purl::parse(body, source)?, source)
}
```

The sole production caller owns a `Vec<AddRequest>` and overwrites it
on the next line, so `std::mem::take` hands it over at no cost. The
pull request reports that `pnpm add lodash@4 react@18 express@4.18.2`,
measured with a counting allocator, fell from four allocations to one.

The chain is three frames deep — the argument-handling caller, the
plan's `parse`, and `parse_specifier` — and the middle frame is what
makes this rule's analysis more than a per-call-site check; see
[Propagating along a chain](#propagating-along-a-chain).

### Recording a completed task

[`pnpm/pnpm#15076`](https://github.com/pnpm/pnpm/pull/15076),
condensed:

```rust
pub fn record_passed(&self, key: &TaskKey, …) -> miette::Result<()> {
    // …
    if !writer.completed.insert(key.clone()) {   // clone escapes into the set
        return Ok(());
    }
    // …
    writer.completed.remove(key);                // second use: rollback
}
```

Both call sites read:

```rust
let key = TaskKey { project: node.project.clone(), task_name: node.task_name.clone() };
task_run_state.record_passed(&key, node, workspace_root);   // never read again
```

`TaskKey` owns a `PathBuf` and a `String`, so the clone is two
allocations for every task a recursive run records.

## Why the sibling rule does not cover these

`perfectionist::needless_borrowed_parameters` applies the gates below,
and both cases fail the first two — which is what makes this a
systematic gap rather than a near-miss.

| Gate | `parse_specifier` | `record_passed` |
| --- | --- | --- |
| The pointee has a recognised owned counterpart: `str`, a slice, or the `Path` / `OsStr` / `cstr_type` diagnostic items | `AddRequest` is a plain ADT → no counterpart | `TaskKey` is a plain ADT → no counterpart |
| The parameter is referenced exactly once, and that use is the conversion | `.selector()` **and** `.clone()` | `.clone()` **and** `remove(key)` |
| The conversion is unconditional | passes, though the clone is conditional — see below | fails: inside an `if` condition |

The first gate is the wider of the two: it confines the sibling rule
to the standard library's borrowed/owned pairs, and every
user-defined `Clone` type — which is what a real codebase clones —
falls outside it. The second confines it to parameters with no
borrowing use at all, while both cases here borrow *and* clone.

The third gate is where the two rules genuinely disagree rather than
merely differ in reach, and it is the subject of
[Why clause 4 is load-bearing](#why-clause-4-is-load-bearing).

That row carries a caveat, because the proposal got it backwards and
an implementer reusing the walker would inherit the mistake.
`parse_specifier` clones inside a `let … else` diverging block, which
is a conditional path — yet the sibling rule fires on that shape
today. Its unconditionality check walks up to the enclosing item
looking for an `if`, `match`, loop, closure, or short-circuiting `&&`
/ `||` **expression**, and a `let … else` puts no such expression
between the clone and the item. Reduced to the crate below and run
through `cargo dylint`, the rule warns on `in_let_else` and stays
silent on `in_if`:

```rust
pub fn in_let_else(flag: Option<u8>, name: &str) -> Option<String> {
    let Some(_n) = flag else {
        return Some(name.to_owned());   // warns, though conditional
    };
    None
}

pub fn in_if(flag: bool, name: &str) -> Option<String> {
    if flag {
        Some(name.to_owned())           // silent, as documented
    } else {
        None
    }
}
```

So the gate is narrower than its stated intent, which is the sibling
rule's own defect to fix rather than this rule's to work around. It
matters here only as a warning: this rule inherits neither the check
nor its gap, since clause 4 replaces the unconditionality argument
outright.

## Why a sibling and not a config knob

The two rules sit at different points on one trade-off curve, with
different soundness arguments:

- **`needless_borrowed_parameters`** requires an unconditional
  conversion, so the change is never worse and **no caller analysis is
  needed**. Cheap, callee-local, sound on its own.
- **`cloned_borrowed_parameter`** permits conditional clones, so the
  change *can* be worse, so it must prove that **every call site owns
  the value and drops it**. That proof reaches outside the callee.

Neither subsumes the other. Relaxing the third gate on the existing
rule would silently void its soundness argument, and the call-site
check is the price of the conditional cases — which is where both real
findings live.

## What to lint

Flag a parameter `p: &T` when all of:

1. `T: Clone` and `T` is not `Copy`. Skip `Rc` and `Arc`: cloning one
   bumps a refcount rather than copying the pointee, so owning it
   saves nothing and a caller that keeps its handle needs one of its
   own. This is the reasoning
   [`src/rules/cloning_getter.rs`](../src/rules/cloning_getter.rs)
   already applies to a ref-counted field.
2. Some reachable path clones `p` and the clone **escapes**: returned,
   stored in a value that is returned, inserted or pushed into a
   collection that outlives the call, or stored in `self`.
3. Every other use of `p` is a borrow. Borrowing uses do not
   disqualify.
4. The callee has at least one call site in the crate, and **every**
   one of them passes a value it owns and does not read afterwards.

### Why clause 2 excludes a clone that stays local

A clone the callee drops before returning is a different defect with a
different fix: delete the clone and use the borrow. Changing the
signature for it would move work onto the callers to fix something the
callee could fix alone. The one shape that resists this reading — a
local clone that is *mutated* rather than merely read, where ownership
really is needed — is a candidate for a later widening of clause 2,
not for the first implementation.

### Why clause 4 asks for two things

The "owns the value" half is the soundness condition: a caller holding
a borrow would have to clone, and for a conditional clone that can be a
net loss.

The "does not read afterwards" half is not needed for soundness. A
caller that keeps reading its value writes `f(x.clone())` and pays
exactly what the callee used to pay — a wash, not a loss. Requiring
the drop is what makes every flagged site a strict improvement rather
than a rearrangement, and that is the only reason it is there.

### Exemptions

Test code and build scripts, by default, for the reason the sibling
rule gives: neither collects on the clone the owned signature saves,
and a test's call sites are exactly the ones holding borrows, so the
trade is a straight loss there. Recognised by `crate::test_code` and
`crate::cargo_target`, per
[Recognising test-exclusive code](./IMPLEMENTATION_CONVENTIONS.md#recognising-test-exclusive-code).

Exempt unconditionally, because the signature is not free to change:

- A method whose signature is fixed by a trait — the same exemption
  the sibling rule applies.
- A function whose `DefId` is mentioned anywhere outside a call
  position, since it may be used as a `fn` pointer or passed as a
  callback, where the signature is fixed by whatever consumes it.
- A parameter with an explicit named lifetime, which may tie it to
  another parameter or the return type.
- An item reachable from outside the crate, whose call sites clause 4
  cannot see. See [Scope](#scope).

Proc-macro-synthesised nodes, per
[Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).

## Examples

**Avoid:** the clone escapes into the map, and the caller's `key` dies
on the next line.

```rust
fn remember(&mut self, key: &Key, value: u32) {
    if self.seen.insert(key.clone()) {
        self.map.insert(key.clone(), value);
    }
}

fn caller(&mut self) {
    let key = Key::new("a", "b");
    self.remember(&key, 1);
}
```

**Prefer:**

```rust
fn remember(&mut self, key: Key, value: u32) {
    if self.seen.insert(key.clone()) {
        self.map.insert(key, value);
    }
}

fn caller(&mut self) {
    self.remember(Key::new("a", "b"), 1);
}
```

**Not flagged:** a caller reuses `key` after the call, so the owned
signature would force it to clone anyway — the very shape clause 4
exists to rule out.

```rust
fn caller(&mut self) {
    let key = Key::new("a", "b");
    self.remember(&key, 1);
    self.log(&key);
}
```

**Not flagged:** the clone never leaves the body, so the fix is to
drop the clone rather than to change the signature.

```rust
fn describe(&self, key: &Key) -> usize {
    let owned = key.clone();
    owned.name().len()
}
```

**Not flagged:** `Arc` is exempt under clause 1 — the clone bumps a
refcount, and a caller that keeps its handle needs one of its own.

```rust
fn register(&mut self, handle: &Arc<Session>) {
    self.sessions.push(handle.clone());
}
```

## Suggested fix

Change the parameter to `p: T`, drop the now-redundant clone, and drop
the `&` at every call site. A caller whose variable is dead after the
call but still in scope reaches for `std::mem::take`, which needs
`T: Default` — so that rewrite is a note for the reader, not something
the lint should emit as a suggestion.

A structured suggestion is worth emitting only for the narrow case
where every call site passes `&<temporary>` and the fix really is
"delete the `&`". Anything wider is a multi-file rewrite whose caller
side the lint cannot render reliably, so it should emit the
diagnostic, a note listing the call sites it checked, and no
suggestion. `Applicability::MaybeIncorrect` in either event: removing
the clone changes the expression's type at the use site, which may
cascade into inference changes the lint cannot verify.

## Configuration

```toml
["perfectionist::cloned_borrowed_parameter"]
# Whether test code / a build script is exempt. Both default to
# `true`; see "Exemptions" above.
exempt_tests = true
exempt_build_scripts = true
```

The same two knobs the sibling rule has, with the same meaning, so a
project that has taken a position on one rule has taken it on both.

The knobs the proposal suggested are deliberately absent, because each
would exist only to turn soundness off:

- **An opt-in for `pub` items.** A crate-local pass cannot see an
  externally reachable item's call sites, so clause 4 is unprovable
  there; a knob that enabled it anyway would let the rule fire on
  evidence it does not have. See [Scope](#scope).
- **A dynamic-edge mode.** Where a call's callee is not statically
  known — a trait method reached through a generic or a `dyn`
  receiver, a closure, a `fn` pointer — there is no edge for the
  fixpoint to follow, so the parameters feeding it retract and the
  rule stays quiet. A knob that assumed such an edge harmless would be
  guessing about bodies the pass never looked at. A callee that is
  itself reached that way is already out of reach under
  [Exemptions](#exemptions).

## Implementation notes

The escape and use classification is the same `LateLintPass`
machinery the sibling rule already has: `check_fn`, typeck results,
param-binding resolution, an HIR visitor over the uses, the
test/build-script exemption, config plumbing.

Clause 4 is the new work.

### Propagating along a chain

Clause 4 reads as a per-call-site check, and for a leaf it is one. It
becomes a fixpoint because of forwarding: when `f(p: &T)` passes `p`
along to `g(&T)`, `f` is a caller that *holds a borrow* today and
would become an owning caller the moment its own parameter is taken by
value. The
[package-specifier chain](#package-specifier-parsing) is exactly this
shape, which is why fixing its innermost frame alone was not enough.

So the property is a **greatest fixpoint**: start every eligible
parameter optimistic, retract on a call site that provably cannot give
up ownership, and iterate until nothing changes. Each parameter
retracts at most once, so the work is bounded by the call graph's
edges rather than by its paths.

The shape to avoid is recursive descent into callees, which expands
the *call tree*: a function reachable by *n* paths is re-analysed *n*
times. Memoised per-function summaries — one entry per parameter,
"does this need owning?" — avoid that, and recursion needs no special
case, since a greatest fixpoint converges downward through a cycle on
its own.

**A depth limit would be the wrong bound.** With memoised summaries
nothing is descended into twice, so a limit buys no time on an
already-linear analysis while making the findings depend on call-graph
shape: extracting a helper would push a fact past the limit and
silently change what the rule reports. Non-determinism under
refactoring is a poor property for a lint. The bound that belongs
here is on *reporting* — fire only where every call site is
visible — which is what [Scope](#scope) already imposes.

A chain also shapes the diagnostic: the frames have to change
together, so a report on one frame should name the others rather than
read as an isolated finding.

### Scope

A `LateLintPass` sees one crate, so the fixpoint stops at the crate
boundary, and the rule can only fire where that boundary contains
every call site. Both motivating cases sit well inside it.

The boundary to test is **effective visibility**, not the `pub`
keyword: what matters is whether an item is reachable from outside the
crate, and `rustc_middle`'s `effective_visibilities` query is the
thing that answers it. Verify what that query returns for a binary
crate before relying on it — a `pub` item in a `bin` target has no
out-of-crate callers, and whether the query says so is a claim to
check against the compiler rather than to assume.

### Difficulty

**Hard**, and the analysis is the cheap half. Two things make it so.

The first is that the trigger is not local to one body — the same
thing that makes [`manual-lazy-init.md`](./manual-lazy-init.md) hard —
and here it is not local to one *pair* of bodies either, because of
forwarding.

The second is calibration. Escaping clones are common and frequently
correct:

```rust
fn insert(&mut self, k: &Key) { self.map.insert(k.clone(), v) }
```

is right whenever callers reuse `k` in a loop, and flagging it would
push people toward a signature that forces every caller to clone
anyway. Clause 4 is what rules that out, which is precisely why it
cannot be dropped to make the rule cheaper.

A conservative first implementation, staged so that each stage ships
on its own:

1. **No fixpoint.** Fire only where the callee is not externally
   reachable, has at least one call site, and *every* call site
   passes `&<temporary>` — an argument `&e` whose `e` the caller
   constructs in the argument position and therefore drops at the end
   of the statement. An owned temporary is provably dead after the
   call, so this stage needs no liveness analysis and no summaries.
   Restrict clause 2 to a clone that is a direct sub-expression of the
   returned value or a direct argument of a call, rather than one
   traced through locals.
2. **Liveness.** Extend the call-site check to `&local` where `local`
   is not read after the call. This is what the
   [`record_passed`](#recording-a-completed-task) call sites need, and
   the reassign-after-call shape that `std::mem::take` serves falls
   out of the same analysis.
3. **Summaries.** Add the per-function fixpoint so a forwarded
   parameter converges, which is what the
   [package-specifier chain](#package-specifier-parsing) needs.

## Out of scope

`Vec<&T>` → `Vec<T>` is a different transformation and must not be
flagged here. It changes the element type rather than the parameter's
ownership, so a caller holding a `Vec<&T>` has to build a fresh vector
— *n* clones. That strengthens the caller's obligation *n*-fold
instead of moving a single value, so the cost model above does not
apply to it.

## Default state

Active by default. The trigger is fully verified rather than
heuristic: clause 4 establishes that no call site regresses, so there
is no class of caller the rule quietly trades against, and no neutral
baseline configuration to omit. The reach that *would* be presumptuous
— an item whose callers live outside the crate — is excluded by
[Scope](#scope) rather than by leaving the rule off.

## Interaction with clippy and sibling rules

None of these clippy lints fires on either motivating case, and the
lint group each sits in is noted so a reader can tell whether their
project runs it at all.

- **`clippy::needless_pass_by_value`** (`pedantic`) covers the
  *opposite* direction: a by-value parameter that is never consumed.
  Per
  [Mirror the Clippy name only for a genuine refinement](./IMPLEMENTATION_CONVENTIONS.md#mirror-the-clippy-name-only-for-a-genuine-refinement),
  a rule pointing the other way must not borrow its name — which is
  why neither this rule nor its sibling does.
- **`clippy::redundant_clone`** (`nursery`) flags a clone of an
  **owned** value that is **dropped without further use**. Here the
  receiver is a borrow and the clone escapes, so it misses on both
  halves.
- **`clippy::unnecessary_to_owned`** (`perf`) fires where the owned
  value is only borrowed again afterwards; here it is genuinely
  stored.
- **`clippy::ptr_arg`** (`style`) rewrites `&Vec<T>` / `&String` /
  `&PathBuf` to `&[T]` / `&str` / `&Path`. Orthogonal: it changes
  which borrowed type the parameter takes, not whether it is borrowed.
- **`perfectionist::needless_borrowed_parameters`** is the sibling
  this rule was carved out of; the two are configured alike and their
  triggers are disjoint, since that rule requires the parameter's only
  use to be an unconditional conversion and this one requires a
  borrowing use or a conditional clone.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
