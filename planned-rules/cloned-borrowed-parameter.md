# `cloned_borrowed_parameter`

**Source:** project convention, distilled from two pnpm changes —
[`pnpm/pnpm#15001`](https://github.com/pnpm/pnpm/pull/15001) and
[`pnpm/pnpm#15076`](https://github.com/pnpm/pnpm/pull/15076) — that the
implemented `perfectionist::needless_borrowed_parameters`
([`src/rules/needless_borrowed_parameters.rs`](../src/rules/needless_borrowed_parameters.rs))
cannot reach. Between them those two changes fix four callees, and
[Motivating cases](#motivating-cases) works from their merged diffs
rather than from a description of them. How many rules the proposal
becomes was left to the implementer;
[Decomposition](#decomposition) records what was decided and why.

## Statement

A function takes a parameter by shared reference and, on some path,
has to produce an owned copy of it — because something it calls wants
the value, not a loan. Every call site already owns such a value and
drops it. Every call therefore pays for a copy nobody needed: the
caller built the value, lent it, the callee duplicated it, and the
caller dropped the original.

Taking the parameter by value moves the caller's value straight in.

### Why this name

The proposal
([`KSXGitHub/perfectionist#471`](https://github.com/KSXGitHub/perfectionist/issues/471))
called this rule `copied_lent_parameter` and said the name was a
placeholder. That one has two problems. *Lent* is not the word Rust
uses for the callee's side of a shared reference — the language, the
compiler's diagnostics, and the sibling rule all say **borrowed**. And
*copied* reads as the `Copy` trait, which the trigger explicitly
excludes: [clause 1](#what-to-lint) requires `Clone` and **not**
`Copy`.

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
body that mixes borrows with one clone is exactly the shape this rule
is for.

### Why clause 4 is load-bearing

That trade holds only while the clone is unconditional. When the
callee clones on some paths and not others, a caller holding a borrow
pays for a clone the callee might never have made. In
[`parse_specifier`](#the-package-specifier-chain), such a caller would
clone for *every* selector, including the purl ones the borrowed
signature never cloned — measured at
[three allocations where there were none](#half-the-chain-is-worse-than-none-of-it).

This is why the sibling rule's unconditional gate is not conservatism
for its own sake: it is the precondition that lets that rule stay
callee-local and skip caller analysis entirely. This rule admits
conditional clones, so it has to buy the same guarantee elsewhere — by
proving that no caller holds a borrow in the first place. That proof
is [clause 4](#what-to-lint), and dropping it to make the rule cheaper
would leave a rule that is wrong about the very cases it exists to
catch.

## Motivating cases

Four callees across two crates. They are worth reading together,
because no two of them are the same shape, and the spread is what
sets the rule's scope.

| Callee                        | Why an owned copy is needed                        | What the call sites look like                  |
| ----------------------------- | -------------------------------------------------- | ---------------------------------------------- |
| `VerdictCache::record`        | consumed by `Value::Object`, then dropped          | every site passes `&<temporary>`               |
| `TaskRunState::record_passed` | inserted into a set, on every call that gets there | an owned local, reused afterwards in two tests |
| `parse_specifier`             | returned, from a `let … else` branch               | forwarded from `parse`                         |
| `PackageSpecifierPlan::parse` | **nothing in its body copies anything**            | a field behind `&mut`, overwritten later       |

Only `parse_specifier` copies on *some* paths. The other two that copy
at all do so on every call that reaches the copy, which matters for
[what keeps the sibling rule quiet](#why-the-sibling-rule-does-not-cover-these).

### A copy that never leaves the body

`VerdictCache::record`, from
[`pnpm/pnpm#15076`](https://github.com/pnpm/pnpm/pull/15076):

```rust
pub(crate) fn record(&self, hash: &str, policy: &Map<String, Value>) {
    let policy_json = Value::Object(policy.clone()).to_string();
    // …
}
```

The clone does not escape: `Value::Object` consumes it, `to_string`
reads it, and the `Value` dies in that statement. It is still a clone
the caller paid for, because `Value::Object` takes a `Map` **by
value** and a borrowed parameter cannot supply one. The fix is
`policy: Map<String, Value>` and `Value::Object(policy)`.

So the test in [clause 2](#what-to-lint) is not whether the copy
*escapes* but whether it is *consumed*. Escaping is one way to be
consumed, and this case is why the wider test is the right one: every
call site here already passes `&<temporary>`, so this is the cheapest
finding of the four, and an escape-only rule would miss it.

### A copy whose removal restructures the body

`TaskRunState::record_passed`, from the same pull request:

```rust
pub fn record_passed(&self, key: &TaskKey, …) -> miette::Result<()> {
    let mut writer = self.writer.lock().expect("…");
    if writer.file.is_none() {
        return Ok(());                  // journalling is off, nothing to record
    }
    if !writer.completed.insert(key.clone()) {   // speculative insert
        return Ok(());                  // already recorded
    }
    // … build the record, serialize it, write one journal line …
    if let Err(error) = result {
        // … if the journal has become unavailable, give up quietly …
        writer.completed.remove(key);   // second use: roll the insert back
        return Err(error).into_diagnostic().wrap_err_with(|| /* … */);
    }
    Ok(())
}
```

The clone is **not** conditional. It sits in an `if` *condition*, so
every call that gets past the journalling check pays for it. What the
`if` decides is whether the set keeps the clone, not whether the clone
happens.

Taking `key` by value leaves only one `insert`, so the speculative
insert with a rollback had to become a `contains` check with the
insert moved to the success path. That is a behaviour-preserving
restructure of the control flow, and the lint must not attempt it;
see [Suggested fix](#suggested-fix).

Two of its call sites are tests that reuse the key, and the change
gave them a `.clone()` each. Paying a clone in a test to remove one
from a recursive run is the trade this rule exists to make, and it is
why [clause 4](#what-to-lint) asks only about production call sites.

`TaskKey` owns a `PathBuf` and a `String`, so the clone was two
allocations per task recorded.

This callee is also the reason the crate boundary is tested by
*effective visibility*: `record_passed` is written `pub`, but its
module is declared `mod cli_args;` without `pub`, so nothing outside
the crate can call it. A rule keyed on the `pub` keyword would have
skipped it. See [The visibility bound](#the-visibility-bound).

### The package-specifier chain

[`pnpm/pnpm#15001`](https://github.com/pnpm/pnpm/pull/15001) is three
frames, and no single frame explains it.

```rust
fn parse_specifier(request: &AddRequest) -> Result<ParsedSpecifier> {
    let specifier = request.selector();          // borrowing use
    // …
    let Some(body) = purl::strip_scheme(specifier) else {
        return Ok(ParsedSpecifier::Node(request.clone()));   // copy returned
    };
    parse_purl(&Purl::parse(body, source)?, source)
}
```

The innermost frame is the ordinary shape: a conditional clone that
is returned, beside a borrowing use.

The middle frame is the interesting one. It read:

```rust
pub(crate) fn parse(package_names: &[AddRequest]) -> Result<Self> {
    // …
    for package_name in package_names {
        match parse_specifier(package_name)? { /* … */ }
    }
    // …
}
```

**Nothing in that body copies anything.** It iterates a borrowed
slice and hands each `&AddRequest` to `parse_specifier`. It is in
this rule's reach only because its callee's parameter must be owned,
which makes the *elements* of `package_names` values it must own —
so the obligation arrives through the call graph, not from a clone
in view. That is [clause 2](#what-to-lint)'s second limb, and it is
why the summary carries a projection and not just a flag per
parameter; see
[What the summary computes](#what-the-summary-computes).

The outermost frame supplies the owned value:

```rust
fn route_package_specifiers(args: &mut AddArgs) -> miette::Result<…> {
    let plan = PackageSpecifierPlan::parse(std::mem::take(&mut args.package_names))?;
    check_specifier_combination(args, &plan)?;
    args.package_names = plan.node_packages;    // overwritten, not read
}
```

It owns the `Vec<AddRequest>`, but through a `&mut` parameter, so it
cannot move out of it — `std::mem::take` is the only rewrite
available, not a stylistic choice. What makes the value free to give
away is that the field is **written before it is next read**, which
is the liveness question clause 4 actually asks.

The pull request reports that `pnpm add lodash@4 react@18
express@4.18.2`, measured with a counting allocator, fell from four
allocations to one. It also took the middle frame one step further
than this rule would, to `impl IntoIterator<Item = AddRequest>`; see
[Out of scope](#out-of-scope).

### Half the chain is worse than none of it

Changing `parse_specifier` alone is not a smaller improvement. It is a
regression, and the shape says why: with `parse` still holding
`&[AddRequest]`, the only way to call a by-value callee is
`parse_specifier(n.clone())`, so every element is copied — including
the purl ones the borrowed signature never copied at all.

Reduced to a three-selector workload under a counting allocator, with
the container built outside the measured region so only the copies
show:

| Variant                                      | all purl | all npm |
| -------------------------------------------- | -------- | ------- |
| as written (borrowed, inner copy)            | 0        | 3       |
| this rule applied to `parse_specifier` alone | 3        | 3       |
| both frames, `Vec<AddRequest>`               | 0        | 0       |
| both frames, `impl IntoIterator`             | 0        | 0       |

Two things follow. The first is that
[clause 4](#what-to-lint) has to be what stops the middle row from
ever being suggested, and it is: `parse` passes an element of a slice
it only borrows, so it owns nothing, and `parse_specifier` fails
clause 4 until `parse`'s own parameter is owned. The frames become
eligible together or not at all, which is what
[An ineligible frame retracts](#an-ineligible-frame-retracts) makes
explicit.

The second is that the owned pointee is the whole win. `Vec<T>` and
`impl IntoIterator<Item = T>` measure identically, so the iterator
bound the pull request chose is a convenience for its callers rather
than the load-bearing part of the change. This rule suggesting `Vec<T>`
gives up nothing.

## Decomposition

The proposal describes one rule;
[the clarification on it](https://github.com/KSXGitHub/perfectionist/issues/471#issuecomment-5735751449)
leaves the count open and separates what is load-bearing from what is
packaging. This section records which way each choice went, so a
later reader can tell a forced decision from a judgement call.

### The one forced boundary

An unconditional conversion satisfies the cost theorem from the
callee's body alone; a conditional one does not, and needs every call
site proven owning. That changes what the analysis must *compute*, and
no packaging makes it go away. Its one hard consequence: whatever else
happens, the unconditional case must keep a path that never runs the
interprocedural pass, so a project can have the cheap check without
paying for the expensive one.

### Folded into this rule

Same trigger question, same diagnostic, same configuration — splitting
these out would produce rules that always ship and always configure
together:

- **Any `Clone` pointee**, sized or not. Three of the four cases take
  an ADT (`&Map<String, Value>`, `&TaskKey`, `&AddRequest`) and the
  fourth a slice (`&[AddRequest]`), whose fix is `Vec<AddRequest>`.
  The unsized pointees need the borrowed-to-owned type mapping the
  sibling rule already carries, and nothing else.
- **A copy consumed without escaping.** `VerdictCache::record` differs
  from the others only in where its copy dies. Excluding it would have
  dropped the one case a first implementation can reach with no
  interprocedural work at all.
- **Ownership demanded through a projection** — the elements of a
  borrowed slice, and the `impl IntoIterator<Item = &T>` spelling of
  the same thing. The package-specifier chain needs it, and it is one
  more axis on the same summary rather than a second analysis.

### Kept as its own rule

`perfectionist::needless_borrowed_parameters` stays separate. The
forced boundary above does not by itself require a second lint name —
one rule with an internal mode would satisfy it — so the case rests on
what a reader and a project get from the split:

- **Adoption.** The cheap check and the interprocedural one have
  different appetites. A project that will not pay for a whole-crate
  pass can keep the callee-local rule and disable this one, which is a
  `[perfectionist].disable` entry rather than a knob nested inside
  another rule's config.
- **Blast radius.** If the interprocedural half turns out noisy,
  switching it off must not cost the half that is already shipping and
  quiet.
- **Diagnostics.** The sibling points at one parameter and suggests a
  signature. This rule has to name the call sites it proved, and a
  chain's frames have to move together, so its diagnostic is a
  different shape rather than a longer version of the same one.

### Not made a rule at all

The caller-side half of the fix — a place passed by reference and then
overwritten with no intervening read, which becomes a
`std::mem::take` — is ordinary local liveness and could stand alone.
It does not, because on its own it is not actionable: the rewrite only
becomes valid once the callee takes the parameter by value, which is
this rule's conclusion. A lint that fires where the reader cannot act
is noise, so this lands as part of the suggestion instead. See
[Suggested fix](#suggested-fix).

### Precedence over the sibling rule

A parameter can satisfy both triggers: one unconditional clone of a
`&str` into a returned `String`, where every caller owns and drops, is
the sibling's finding and this one's at once. Reported twice, it is
one problem wearing two names.

So this rule stays silent wherever the sibling's predicate holds —
**whether or not the sibling is enabled**. Deferring to the predicate
rather than to the enablement keeps the decision callee-local; firing
here when the sibling is switched off would make one rule's output
depend on another rule's configuration, and would hand a project that
declined the cheap check the expensive rule's version of the very same
finding. A project that wants neither disables both.

## Why the sibling rule does not cover these

`perfectionist::needless_borrowed_parameters` applies the gates below,
and every case fails at least the first two.

| Gate                                                        | `parse_specifier`                    | `record_passed`              |
| ----------------------------------------------------------- | ------------------------------------ | ---------------------------- |
| Pointee has a recognised owned counterpart                  | no: `AddRequest` is a plain ADT      | no: `TaskKey` is a plain ADT |
| Parameter used exactly once, and that use is the conversion | no: also `.selector()`               | no: also `remove(key)`       |
| Conversion is unconditional                                 | no expression says so, but see below | no: an `if` is in the way    |

The recognised counterparts are `str`, a slice, and the `Path` /
`OsStr` / `cstr_type` diagnostic items, so the first gate is about
reach rather than soundness: it confines the sibling rule to the
standard library's borrowed/owned pairs, and every user-defined
`Clone` type — which is what a real codebase clones — falls outside
it. **That gate alone settles both columns**, and it is why the
sibling stays quiet even though neither clone is conditional. The
second gate confines the sibling to parameters with no borrowing use
at all, while both cases here borrow *and* clone.
`PackageSpecifierPlan::parse` fails even earlier, since it performs no
conversion for any gate to inspect.

The third gate is a syntactic test, not a semantic one, and the two
come apart. Its check asks whether an `if`, `match`, loop, closure or
short-circuiting operator **expression** sits between the conversion
and the enclosing item — so an `if` *condition*, which always runs,
disqualifies just as an arm does. The sibling's own documentation says
as much. Run over a fixture with a recognised pointee, a single use,
and the conversion in an `if` condition, it stays silent:

```rust
// `needless_borrowed_parameters` does not fire here, although
// `to_owned` runs on every call.
pub fn in_if_condition(name: &str, set: &mut HashSet<String>) -> bool {
    set.insert(name.to_owned())
}
```

So "fails the unconditional gate" never means "the clone is
conditional". Only `parse_specifier`'s is.

The third gate is the [forced boundary](#the-one-forced-boundary), and
it is the subject of
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
does bear on [Precedence](#precedence-over-the-sibling-rule) — the
predicate this rule defers to is the one the sibling actually has, not
the one it means to have — so fixing it there widens this rule here,
which is the correct direction.

## What to lint

Flag a parameter `p` when all of:

1. Its written type is `&T` with an elided lifetime, `T: Clone`, and
   `T` is not `Copy`. `T` may be sized (`TaskKey`, `Map<String,
   Value>`) or unsized (`str`, `[AddRequest]`, `Path`); the suggestion
   names its owned counterpart, which is `T` itself when sized and
   `String` / `Vec<Foo>` / `PathBuf` / `OsString` / `CString` when not.
   Skip `Rc` and `Arc`: cloning one bumps a refcount rather than
   copying the pointee, so owning it saves nothing and a caller that
   keeps its handle needs one of its own. This is the reasoning
   [`src/rules/cloning_getter.rs`](../src/rules/cloning_getter.rs)
   already applies to a ref-counted field.
2. Some reachable path needs an owned `T`, in either of two ways:
   1. the body copies `p` and **consumes** the copy — moves it into a
      returned value, a field of `self`, a collection, or any call
      that takes it by value; or
   2. the body passes a borrow of `p`, or of a place projected from
      `p`, to a callee whose corresponding parameter the summary says
      must be owned.
3. Every other use of `p` is a borrow. Borrowing uses do not
   disqualify.
4. The callee has at least one production call site in the crate, and
   **every** production call site passes a place it owns and does not
   read again — either because the place goes out of scope, or because
   it is overwritten first.
5. The sibling rule's predicate does not already hold, per
   [Precedence](#precedence-over-the-sibling-rule).

### Why clause 2 asks about consumption, not escape

The proposal asked whether the copy *escapes* the call. That is the
wrong line, and `VerdictCache::record` is on the wrong side of it:
its copy dies inside the statement that makes it, yet the caller still
paid for it, because `Value::Object` takes its argument by value and a
borrowed parameter has none to give.

The line that matters is whether anything **takes the copy by value**.
A copy that is only ever *borrowed from* afterwards is a different
defect with a local fix — delete the copy and use the original
borrow — and belongs to
[clippy](#interaction-with-clippy-and-sibling-rules) rather than here.

### Why clause 4 asks about production call sites

The "owns the value" half is the soundness condition: a caller holding
a borrow would have to clone, and for a conditional clone that can be
a net loss.

"Does not read again" is the liveness half, and it is deliberately
about a **place** rather than a variable. `record_passed`'s callers
pass a local that simply dies; in
[`route_package_specifiers`](#the-package-specifier-chain) the place
is a field behind a `&mut` parameter, assigned two statements later.
Both are free to give away, and only the first is a variable going out
of scope. It is not needed for soundness — a caller that reads its
value again writes `f(x.clone())` and pays exactly what the callee
used to pay, a wash rather than a loss — but requiring it is what
makes every flagged site a strict improvement.

**Production** is the third word doing work. Under the default
exemptions a test call site does not veto the rule; it just acquires a
`.clone()` when the fix lands, which is what happened to two of
`record_passed`'s tests. This is the same trade the exemptions
already make in the other direction — a test never collects on the
clone the owned signature saves — so `exempt_tests` and
`exempt_build_scripts` govern both halves at once, and setting either
to `false` makes those call sites count like any other.

### Exemptions

Test code and build scripts, by default, for the reason the sibling
rule gives: neither collects on the clone the owned signature saves,
and a test's call sites are exactly the ones holding borrows.
Recognised by `crate::test_code` and `crate::cargo_target`, per
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
  cannot see. See [The visibility bound](#the-visibility-bound).

Proc-macro-synthesised nodes, per
[Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).

## Examples

**Avoid:** the copy is consumed by an API that takes it by value, and
dies in the same statement. Every call site builds its argument in
place.

```rust
fn record(&self, policy: &Map<String, Value>) {
    let json = Value::Object(policy.clone()).to_string();
    self.write(json);
}

fn caller(&self) {
    self.record(&merge_policies(&self.verifiers));
}
```

**Prefer:**

```rust
fn record(&self, policy: Map<String, Value>) {
    let json = Value::Object(policy).to_string();
    self.write(json);
}

fn caller(&self) {
    self.record(merge_policies(&self.verifiers));
}
```

**Avoid:** the first copy is made on every call and the second under a
condition, and the caller's place is overwritten rather than read
again. `Key` is the project's own type, so clause 5 does not apply —
the sibling rule's first gate does not recognise the pointee.

```rust
fn remember(&mut self, key: &Key, value: u32) {
    if self.seen.insert(key.clone()) {
        self.map.insert(key.clone(), value);
    }
}

fn caller(args: &mut Args, next: Key) {
    store.remember(&args.key, 1);
    args.key = next;                  // overwritten, never read again
}
```

**Prefer:** one copy instead of two, and the caller cannot move out of
a place behind `&mut`, so the rewrite is `std::mem::take`.

```rust
fn remember(&mut self, key: Key, value: u32) {
    if self.seen.insert(key.clone()) {
        self.map.insert(key, value);
    }
}

fn caller(args: &mut Args, next: Key) {
    store.remember(std::mem::take(&mut args.key), 1);
    args.key = next;
}
```

**Not flagged:** the copy is only borrowed from, so the fix is to drop
the copy rather than to change the signature.

```rust
fn describe(&self, key: &Key) -> usize {
    let owned = key.clone();
    owned.name().len()
}
```

**Not flagged:** a caller reads `key` again after the call, so the
owned signature would force it to clone anyway — the shape clause 4
exists to rule out.

```rust
fn caller(&mut self) {
    let key = Key::new("a", "b");
    self.remember(&key, 1);
    self.log(&key);
}
```

**Not flagged:** `Arc` is exempt under clause 1 — the clone bumps a
refcount, and a caller that keeps its handle needs one of its own.

```rust
fn register(&mut self, handle: &Arc<Session>) {
    self.sessions.push(handle.clone());
}
```

**Not flagged:** one unconditional clone, no other use — the sibling
rule's finding, so clause 5 keeps this rule quiet.

```rust
fn store(name: &str, registry: &mut HashMap<String, u32>) {
    registry.insert(name.to_owned(), 0);
}
```

## Suggested fix

Change the parameter to `p: T` and drop the `&` at every call site. A
caller whose place is behind a reference cannot move out of it and
needs `std::mem::take`, which requires `T: Default`; where the place
is a local going out of scope, a plain move does. A caller that
already has the replacement value in hand wants
`std::mem::replace(&mut place, next)` rather than a `take` followed by
an assignment, which would write the place twice.

This caller-side half is where the two rules visibly differ. The
sibling makes no caller-side claim at all: its suggestion rewrites the
signature and the body and leaves every call site alone, under the
help text *take the owned type by value and let callers convert at the
call site*. It can say that because its trade is neutral for the worst
caller. This rule's whole justification is a claim about callers, so
the call sites are part of the finding rather than someone else's
problem.

The callee side is **not** a mechanical edit, and the diagnostic
should not pretend otherwise. Deleting the copy works when the copy
had one consumer. When the borrowed signature was what allowed two —
`record_passed` inserted a clone speculatively and removed the
original on failure — an owned parameter admits only one, and the
body has to be restructured to suit. That restructure is the author's
judgement about behaviour, not a rewrite the lint can author.

So a structured suggestion is worth emitting only where every call
site passes `&<temporary>` and the fix really is "delete the `&`".
Anything wider should emit the diagnostic, a note listing the call
sites it checked and how many are tests, and no suggestion.
`Applicability::MaybeIncorrect` in either event.

One more condition the lint should check before suggesting: a borrow
derived from the parameter must not be live at the point the owned
value is moved. `parse_specifier` gets away with
`let specifier = request.selector();` before
`ParsedSpecifier::Node(request)` only because nothing on that path
reads `specifier` afterwards, so the borrow has ended. Move the value
while such a borrow is still live and the result does not compile.

## Configuration

```toml
["perfectionist::cloned_borrowed_parameter"]
# Whether test code / a build script is exempt. Both default to
# `true`, and each governs two things: whether such a body is flagged,
# and whether a call site there can veto clause 4. See "Why clause 4
# asks about production call sites" above.
exempt_tests = true
exempt_build_scripts = true
```

The same two knobs the sibling rule has, so a project that has taken a
position on one rule has taken it on both.

The knobs the proposal suggested are deliberately absent, because each
would exist only to turn soundness off:

- **An opt-in for `pub` items.** A crate-local pass cannot see an
  externally reachable item's call sites, so clause 4 is unprovable
  there; a knob that enabled it anyway would let the rule fire on
  evidence it does not have. See
  [The visibility bound](#the-visibility-bound).
- **A dynamic-edge mode.** Where a call's callee is not statically
  known — a trait method reached through a generic or a `dyn`
  receiver, a closure, a `fn` pointer — there is no edge for the
  fixpoint to follow, so the parameters feeding it retract and the
  rule stays quiet. A knob that assumed such an edge harmless would be
  guessing about bodies the pass never looked at. A callee that is
  itself reached that way is already out of reach under
  [Exemptions](#exemptions).

## Shared infrastructure: the ownership summary

Clause 4 is not this rule's private machinery, and building it inside
whichever rule lands first would be a mistake. It is a worklist over
the crate's call graph answering one question per parameter — must
this be taken by value? — and the same worklist over a different
lattice answers the weakening question described under
[Out of scope](#out-of-scope). Factor it into a crate-internal
`ownership_summary` module, per
[Notes on cross-rule dependencies](../CLAUDE.md#notes-on-cross-rule-dependencies),
and let the rules consume it.

### What the summary computes

One entry per function, and within it one answer per parameter **and
per projection of that parameter the body hands out**. A flag per
parameter is not enough: `PackageSpecifierPlan::parse` never needs its
`package_names` argument itself, only owned *elements* of it, and a
domain that cannot say so has nothing to propagate from
`parse_specifier` back up the chain. Slice and iterator elements are
the projection the motivating cases need; fields are the obvious next
one. A rule reads a summary. It does not walk callees itself.

### Propagating along a chain

Clause 4 reads as a per-call-site check, and for a leaf it is one. It
becomes a fixpoint because of forwarding: when `f(p: &T)` passes `p`
along to `g(&T)`, `f` is a caller that *holds a borrow* today and
would become an owning caller the moment its own parameter is taken by
value. The [package-specifier chain](#the-package-specifier-chain) is
exactly this shape, which is why fixing its innermost frame alone was
not enough.

So the property is a **greatest fixpoint**: start every eligible
parameter optimistic, retract on a call site that provably cannot give
up ownership, and iterate until nothing changes. Each parameter
retracts at most once, so the work is bounded by the call graph's
edges rather than by its paths.

The shape to avoid is recursive descent into callees, which expands
the *call tree*: a function reachable by *n* paths is re-analysed *n*
times. Memoised per-function summaries avoid that, and recursion needs
no special case, since a greatest fixpoint converges downward through
a cycle on its own.

**A depth limit would be the wrong bound.** With memoised summaries
nothing is descended into twice, so a limit buys no time on an
already-linear analysis while making the findings depend on call-graph
shape: extracting a helper would push a fact past the limit and
silently change what the rule reports. Non-determinism under
refactoring is a poor property for a lint. The bound that belongs here
is on *reporting*, which is the next section.

### An ineligible frame retracts

A frame this rule may not touch — reachable from outside the crate,
signature fixed by a trait, used as a `fn` pointer, carrying a named
lifetime, macro-generated, or exempt test code — must **retract in the
summary**, not merely go unreported. The distinction is the difference
between a correct rule and a harmful one.

Take the package-specifier chain and suppose `parse` were reachable
from outside the crate. Its parameter can then never be owned. If that
only meant "do not report `parse`", the fixpoint would still be
carrying the optimistic assumption that `parse` becomes an owning
caller, would conclude that `parse_specifier` is eligible, and would
report it alone — which is
[the row that costs three allocations](#half-the-chain-is-worse-than-none-of-it).
Retraction propagates instead: `parse` cannot be owned, so the element
it lends cannot be owned, so `parse_specifier`'s parameter retracts
too, and the chain goes quiet as a whole.

The same holds for the dynamic edges under
[Configuration](#configuration). Retraction is the single mechanism;
"ineligible" is just another reason to retract.

It follows that the finding is the **chain**, not the frame. A report
should name every frame it expects to move and say that they move
together, because a reader who applies a strict subset makes the code
worse.

### The visibility bound

A `LateLintPass` sees one crate, so the fixpoint stops at the crate
boundary, and a rule built on it can only fire where that boundary
contains every call site.

The boundary to test is **effective visibility**, not the `pub`
keyword. `TaskRunState::record_passed` is the case that settles it: it
is written `pub`, and it is unreachable from outside its crate anyway,
because the module holding it is declared `mod cli_args;` with no
`pub`. A `pub`-keyword test would have skipped a real finding.
`rustc_middle`'s `effective_visibilities` query is the thing that
answers the question properly — verify what it returns for a binary
crate before relying on it, since a `pub` item in a `bin` target has
no out-of-crate callers either, and whether the query says so is a
claim to check against the compiler rather than to assume.

## Implementation notes

Everything outside clauses 2.2 and 4 is the `LateLintPass` machinery
the sibling rule already has: `check_fn`, typeck results,
param-binding resolution, an HIR visitor over the uses, the
test/build-script exemption, config plumbing. The borrowed-to-owned
type mapping clause 1 needs for an unsized pointee is that rule's too,
and belongs in a shared helper rather than a second copy.

A chain shapes the diagnostic as well as the analysis: the frames have
to change together, so a report on one frame should name the others
rather than read as an isolated finding.

### Difficulty

**Hard**, and the analysis is the cheap half. Two things make it so.

The first is that the trigger is not local to one body — the same
thing that makes [`manual-lazy-init.md`](./manual-lazy-init.md) hard —
and here it is not local to one *pair* of bodies either, because of
forwarding.

The second is calibration. Copies that are consumed are common and
frequently correct:

```rust
fn insert(&mut self, k: &Key) { self.map.insert(k.clone(), v) }
```

is right whenever callers reuse `k` in a loop, and flagging it would
push people toward a signature that forces every caller to clone
anyway. Clause 4 is what rules that out, which is precisely why it
cannot be dropped to make the rule cheaper.

A conservative implementation, staged so that each stage ships on its
own and each is answerable by a case above:

1. **No summaries, no liveness.** Clause 2.1 only, with the copy a
   direct argument of a by-value call or a direct sub-expression of
   the returned value; callee not externally reachable; every
   production call site passes `&<temporary>`, which is provably dead
   after the call. Catches `VerdictCache::record`.
2. **Place liveness.** Extend the call-site check to `&place` — a
   local, or a projection behind a `&mut` — that the caller does not
   read again. Catches `record_passed`, and the outer frame of the
   package-specifier chain.
3. **Summaries.** Stand up the `ownership_summary` module and clause
   2.2, so a forwarded parameter converges. Catches
   `parse_specifier`.
4. **Projections.** Carry per-projection answers in the summary, so
   ownership demanded of an element reaches the parameter that yields
   it. Catches `PackageSpecifierPlan::parse` and closes the chain.

Stage 1 is worth shipping by itself: it needs none of the
interprocedural machinery and still finds a real allocation.

## Out of scope

**`Vec<&T>` → `Vec<T>`** is a different transformation and must not be
flagged here. It changes the element type rather than the parameter's
ownership, so a caller holding a `Vec<&T>` has to build a fresh vector
— *n* clones. That strengthens the caller's obligation *n*-fold
instead of moving a single value, so the cost model above does not
apply to it.

**Weakening a slice parameter to an owned iterator bound** —
`&[T]` → `impl IntoIterator<Item = T>`, which is what
[`pnpm/pnpm#15001`](https://github.com/pnpm/pnpm/pull/15001) actually
wrote where this rule would have suggested `Vec<T>` — is a separate
question that was discussed alongside the proposal and has not been
filed. It runs the same worklist over a different lattice, which is
why [the summary](#shared-infrastructure-the-ownership-summary) is
factored out rather than written into this rule. This rule suggests
the owned pointee and stops there.

## Default state

Inactive by default, and what decides it is which way the analysis
fails rather than whether it is exact.

A rule resting on a denylist gets the opposite answer: a type it
misclassifies simply goes unflagged, so being wrong costs findings
rather than trust, and `perfectionist::implicit_effectful_default`
ships active on exactly that kind of imperfection. Clause 4 is not
like that. It is a whole-crate interprocedural summary, and getting
it wrong in the permissive direction — a call site missed, a frame
that should have [retracted](#an-ineligible-frame-retracts) and did
not, a place the liveness half read as dead — yields no silence. It
yields a *finding*, and acting on that finding is a measured
pessimisation:
[three allocations where there were none](#half-the-chain-is-worse-than-none-of-it).

The tempting argument the other way is that the trigger is verified
rather than heuristic, since clause 4 establishes that no production
call site regresses. That holds of the *specification* and says
nothing about the implementation. For a syntactic rule the two are close
enough to conflate; for a greatest fixpoint over a crate they are
not.

The remedy compounds it. Every finding rewrites the callee's
signature and each of its call sites, with no autofix — a larger and
less reversible edit than a lint usually asks for, and one to opt
into rather than to meet on a first run.

Worth revisiting against real findings, and the
[staged plan](#difficulty) makes that a graded question rather than a
single one: stages 1 and 2 need none of the interprocedural
machinery, so the reasoning above bears on them far more weakly than
on stages 3 and 4.

## Interaction with clippy and sibling rules

None of these clippy lints fires on any of the motivating cases, and
the lint group each sits in is noted so a reader can tell whether
their project runs it at all.

- **`clippy::needless_pass_by_value`** (`pedantic`) covers the
  *opposite* direction: a by-value parameter that is never consumed.
  Per
  [Mirror the Clippy name only for a genuine refinement](./IMPLEMENTATION_CONVENTIONS.md#mirror-the-clippy-name-only-for-a-genuine-refinement),
  a rule pointing the other way must not borrow its name — which is
  why neither this rule nor its sibling does.
- **`clippy::redundant_clone`** (`nursery`) flags a clone of an
  **owned** value that is **dropped without further use**. Here the
  receiver is a borrow, so it misses on the first half whatever the
  copy goes on to do.
- **`clippy::unnecessary_to_owned`** (`perf`) fires where the owned
  value is only **borrowed** again afterwards. That is the exact
  complement of [clause 2.1](#what-to-lint), which requires the copy
  to be taken **by value** — so the two partition the space rather
  than overlap, and a copy neither lint claims is one the borrow
  checker already forced.
- **`clippy::ptr_arg`** (`style`) rewrites a `&Vec<T>` / `&String` /
  `&PathBuf` parameter to `&[T]` / `&str` / `&Path`. Since clause 1
  admits those pointees, both lints can speak about one parameter —
  but they do not conflict, because this rule's fix removes the
  reference altogether and `ptr_arg` only flags references. Taking
  `Vec<T>` by value satisfies both at once, and `ptr_arg`'s
  suggestion, applied first, leaves `&[T]` still in this rule's reach.
- **`perfectionist::needless_borrowed_parameters`** is the sibling
  this rule was carved out of; the two are configured alike and their
  triggers are made disjoint by clause 5, per
  [Precedence](#precedence-over-the-sibling-rule).

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
