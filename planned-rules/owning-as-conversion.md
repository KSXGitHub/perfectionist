# `owning_as_conversion`

**Sources:** this repository's own review history.
<https://github.com/KSXGitHub/perfectionist/pull/462#discussion_r4068816189>
raised the case while reviewing `cloning_as_conversion`'s fixture:

> Converting a number to a `String` is technically not an act of
> cloning. But it still allocates.
>
> Should this rule be widen to flag all `as`-conversion that
> allocates? Should this rule be subsequently be renamed? Or was this
> flagged by another rule already?

The answer to the last question is no — see
[What nothing else catches](#what-nothing-else-catches) — and this
file exists because the answer to the first two is "not that rule".

> [!NOTE]
> Two of the siblings this file reasons about are still in flight, so
> the files it names for them do not exist on `master` yet:
> `cloning_as_conversion` in
> <https://github.com/KSXGitHub/perfectionist/pull/462> and
> `borrowing_to_conversion` in
> <https://github.com/KSXGitHub/perfectionist/pull/463>. Nothing here
> should be implemented before the first of those lands, since the
> subsumption argument below is the reason this is a separate rule at
> all.

## Statement

The `as_` prefix promises a *free* conversion: a borrowed view of
something the receiver already holds. A method that takes `&self` and
hands back a freshly allocated value has broken that promise, whatever
it did to produce the value.

**Avoid:**

```rust
impl Person {
    fn as_age_label(&self) -> String {
        self.age.to_string()
    }
}
```

**Prefer:**

```rust
impl Person {
    fn to_age_label(&self) -> String {
        self.age.to_string()
    }
}
```

The body is unchanged, and is meant to be. Nothing about rendering a
`u32` is wrong; what is wrong is the prefix claiming the caller pays
nothing for it.

## Why restrict this?

This is a stylistic preference, not a correctness issue. The method
compiles, returns the right value, and leaks nothing.

The preference is that a caller reads `as_` as a cost signal and
budgets accordingly. `as_` is what lets a reader write
`thing.as_slug()` inside a loop without stopping to think, because
every `as_*` they have met — in `std`, and very nearly everywhere else
(see below) — handed back a view. One method that allocates under that
prefix costs the reader the ability to skim any of them.

`to_` is the prefix for a conversion that costs something, so the fix
is a rename rather than a rewrite, and the rename is free.

## What nothing else catches

Measured before proposing the rule, because a rule nobody needs is
worse than no rule.

- **This plugin.** No registered lint reports
  `fn as_age_label(&self) -> String { self.age.to_string() }`.
  `perfectionist::cloning_as_conversion` requires the body to copy a
  field and the return type to *be* that field's type; a rendered
  `u32` is neither.
- **Clippy.** A scratch crate holding that method under
  `-W clippy::all -W clippy::pedantic -W clippy::nursery
  -W clippy::restriction` drew only `missing_inline_in_public_items`
  and `must_use_candidate`. `clippy::wrong_self_convention` is the
  near miss and does not fire: it checks which *receiver* an `as_*`
  method takes, and `&self` is the one it wants. It never looks at the
  return type.

## How often is this written?

A count over the local crate registry cache: 379 crates, every
`fn as_<name>(&self) -> …` whose return type sits on the same line as
the signature. That undercounts wrapped signatures, so read it as a
floor.

| | count |
|---|---|
| `as_*(&self)` methods found | 1732 |
| returning `String`, `Vec<…>`, `PathBuf`, `OsString`, `CString`, `Box<…>` | **0** |
| returning a `&`-prefixed type | 869 |
| carrying a lifetime in the return type | 346 |
| returning `Option<…>` | 246 |
| returning `Result<…>` | 93 |
| returning `Cow<…>` | 46 |

Zero, out of 1732. The only hits that mention an owning type at all
mention it through a borrow or a lifetime — `&Vec<u8>`,
`Option<&Vec<Self>>`, `&BString`, `TomlString<'s>`, `ZeroVec<'_, T>`.

> [!IMPORTANT]
> This cuts both ways, and the decision to implement should face it.
>
> It says the convention is real and essentially never violated, so a
> rule keyed on it would be near-zero-false-positive. It also says the
> rule will almost never fire on library code, because published
> crates already police their own `as_*` names in review.
>
> The case for shipping anyway is that the corpus is the wrong
> population. It is libraries, where the prefix is part of the public
> API and gets argued over. The violations live in application code
> and in generated code — which is the audience this catalogue was
> built for, and which is where this repository found its own instance
> (`as_age_label`, in `ui/cloning_as_conversion.rs`).
>
> If that argument does not convince, the honest outcome is to close
> this file rather than implement it.

## What to lint

A `LateLintPass` over an inherent method where all of the following
hold:

1. **The name begins with `as_`**, with `as_` a proper prefix — the
   same test `cloning_as_conversion` applies, which admits `as_name`
   and rejects `ascii_name`.
2. **The receiver is `&self`, and there are no other parameters.**
   A method taking arguments is not a conversion of `self`; a method
   taking `self` or `&mut self` is a different mistake.
3. **The return type owns a heap allocation**, per the predicate
   below.

### The owning predicate

`owns_allocation(ty)`, in the order the cases are tried:

- **A reference, a raw pointer, or any `Copy` type** → no. A shared
  reference is itself `Copy`, so a return type that is already a
  borrow falls out here.
- **A type carrying a lifetime argument** → no. `Cow<'_, CStr>`,
  `BorrowedFd<'_>`, `TomlString<'s>` and `ZeroVec<'_, T>` are all free
  to borrow from the receiver, so the prefix's promise is not broken on
  the face of the signature. This case is why the corpus's 346
  lifetime-carrying returns and 46 `Cow`s never reach the allowlist.
- **`Option<T>` or `Result<T, E>`** → recurse into `T` alone. An
  error type is the conversion's failure path, not its product.
- **A known heap-owning `std` type** — `String`, `Vec`, `PathBuf`,
  `OsString`, `CString`, `Box`, `HashMap`, `BTreeMap`, `HashSet`,
  `BTreeSet`, `VecDeque`, `BinaryHeap` → yes.
- **Anything else** → no.

> [!NOTE]
> The last two cases are the contested pair, and the choice between
> them is the main thing this file exists to settle.
>
> An **allowlist** under-covers: a `Bytes`, a `SmallVec`, an
> `IndexMap` is missed, and the list needs extending as the ecosystem
> moves. It never false-positives.
>
> The alternative — **"any ADT that is neither `Copy` nor
> lifetime-carrying"** — covers all of those and needs no list, but
> flags an owned-but-cheap ADT that allocates nothing, and there is no
> way to tell one from the other at the signature.
>
> The allowlist is proposed because the corpus shows the covered types
> are the whole of the real population, and because a rule about a
> naming promise should not guess. It also buys the `Rc` / `Arc`
> exemption for free: neither is on the list, so
> `fn as_shared(&self) -> Rc<String>` never fires, without needing the
> refcount reasoning `cloning_as_conversion` had to spell out.

### Exemptions

- **A trait impl**, because the trait fixes the signature and the impl
  cannot change it. `AsRef::as_ref` is the case that matters.
- **Macro-synthesised methods**, per
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).
- **A method `cloning_as_conversion` already reports**, per the next
  section.

Both of the first two already exist as
`crate::field_copy::eligible_method`, which applies the same
eligibility test on behalf of this family.

> [!NOTE]
> Open: whether test code is exempt.
> `needless-borrowed-parameters.md` exempts it, on the ground that no
> test collects on the copy it saves. The argument does not obviously
> carry: this rule saves nothing at runtime, it protects a reader, and
> a test helper has readers too. Proposed: **no exemption**, so the
> rule needs no `crate::test_code` dependency at all. See
> [Recognising test-exclusive code](./IMPLEMENTATION_CONVENTIONS.md#recognising-test-exclusive-code)
> if that is overruled.

## Interaction with sibling rules

### `perfectionist::cloning_as_conversion` — strict subsumption

This is the decision with the largest blast radius, because it is
about a lint that has already shipped.

**The triggers nest.** Read
[`src/field_copy.rs`](../src/field_copy.rs): a `FieldCopy` is
recognised only where the body's type equals the field's type, the
field is not `Copy`, and the field is not a refcounted handle. The
method's return type is therefore the field's own owned type, so every
site `cloning_as_conversion` reports is also a site where
`owns_allocation` holds. The converse fails at `as_age_label`. So
this rule's trigger strictly contains the sibling's.

Nested triggers mean two diagnostics on every `as_name` unless one
rule stands down. The options:

1. **This rule stands down** where `field_copy` recognises the body.
2. **Widen `cloning_as_conversion`** into a single rule with a
   two-branch help.
3. **Retire `cloning_as_conversion`** into this one.

**Proposed: option 1**, and the reason is the help rather than the
trigger. The two rules can say different things:

| | what it can offer |
|---|---|
| `cloning_as_conversion` | *change the return type* — `&str` for a `String` field, `&Path` for a `PathBuf`, because a borrow of the field would have served |
| `owning_as_conversion` | *change the name* — there is no borrow to offer, because no borrow of a `u32` is a `String` |

A rule that emits "change the return type" for one half of its trigger
and "change the name" for the other half is two rules wearing one
name. Keeping them apart also keeps each name honest under
[Do not over-claim in the name](./IMPLEMENTATION_CONVENTIONS.md#do-not-over-claim-in-the-name).

Option 2 additionally costs a rename of a released lint, which
invalidates consumers' existing `#[allow(perfectionist::…)]`
attributes. Option 3 discards a more specific and more actionable
diagnostic. Neither is free; option 1 is.

### `perfectionist::borrowing_to_conversion`

The suggested fix here is a rename to `to_*`, which lands inside that
rule's territory — it flags a `to_*` that hands back a *borrow*. The
two compose rather than collide: this rule only fires on a method
returning an owned allocation, so the renamed method returns owned and
the sibling has nothing to say about it. Taking this rule's advice can
never trip that one.

### `perfectionist::cloning_getter`

Orthogonal by construction. That rule's `is_getter` rejects any name
beginning with a conversion prefix outright, `as_` among them, so no
method this rule can fire on is a getter as far as that one is
concerned.

## Examples

```rust
struct Person {
    name: String,
    age: u32,
    shared: Rc<String>,
}

impl Person {
    // Bad: allocates under a prefix that promises not to. The fix is
    // the name, not the body.
    fn as_age_label(&self) -> String {
        self.age.to_string()
    }

    // Bad: builds a fresh collection. Nothing here is a view of
    // anything the receiver holds.
    fn as_words(&self) -> Vec<String> {
        self.name.split(' ').map(str::to_owned).collect()
    }

    // Bad: an allocation under an `Option` is still an allocation.
    fn as_initial_label(&self) -> Option<String> {
        self.name.chars().next().map(|c| c.to_string())
    }

    // Good: a borrow, which is what the prefix says.
    fn as_name(&self) -> &str {
        &self.name
    }

    // Good: `Cow` is the honest type for a conversion that is
    // sometimes free, and it carries a lifetime, so the predicate
    // leaves it alone.
    fn as_display_name(&self) -> Cow<'_, str> {
        if self.name.is_empty() { Cow::Borrowed("anonymous") } else { Cow::Borrowed(&self.name) }
    }

    // Not flagged: `Rc` is not on the allowlist, so cloning a handle
    // never reaches the predicate.
    fn as_shared(&self) -> Rc<String> {
        self.shared.clone()
    }

    // Not flagged: reported by `cloning_as_conversion` instead, which
    // can offer the borrowed form this rule cannot.
    fn as_owned_name(&self) -> String {
        self.name.clone()
    }
}
```

## Configuration

None proposed. The trigger is a naming promise, and a project that
disagrees with the promise wants the rule off rather than tuned.

The one knob with an argument behind it is an **extra-types list**,
extending the allowlist with a project's own owning types
(`Bytes`, `SmallVec`, an in-house arena handle). It is left out of the
first implementation deliberately: a knob nobody sets is the shape
[A planning file is a proposal, not a specification](../CLAUDE.md#a-planning-file-is-a-proposal-not-a-specification)
warns about, and the corpus gives no evidence anyone would set this
one. Add it when someone asks.

## Default state

Active by default. The trigger reads a signature and nothing else, and
the corpus above puts its false-positive rate on real published code at
zero out of 1732. Nothing here is advisory or project-dependent enough
to earn `Inactive`, per
[Rule activation model](./IMPLEMENTATION_CONVENTIONS.md#rule-activation-model).

## Autofix

None. The suggestion is a rename, and renaming a method is a
source-breaking change for every caller — including callers outside the
crate being linted, which the pass cannot see. Emit the help text
naming the concrete replacement (`as_age_label` → `to_age_label`) and
leave the edit to a human.

The `as_` → `to_` mapping is mechanical enough that this could be
revisited as a `Suggestion` at `Applicability::MaybeIncorrect`, but the
first implementation should not.

## Implementation notes

A `LateLintPass::check_fn`, reusing what the family already has:

- `crate::field_copy::eligible_method` gives conditions 2 and the
  first two exemptions in one call, and is already written.
- The `as_` prefix test is `cloning_as_conversion`'s `AS_PREFIX`
  constant and the `starts_with` beside it. Two rules wanting it is
  the point at which it moves to `crate::common`, per
  [One rule per file, one `Config` per rule](../CLAUDE.md#one-rule-per-file-one-config-per-rule).
- `owns_allocation` is the only new code. The lifetime case is
  `ty.walk()` for a non-`'static` region, or the ADT's own
  `GenericArgs` for a region argument — verify which before building
  on either, since the two differ on a type whose lifetime appears
  only in a field.
- `is_copy` is `clippy_utils::ty::is_copy`. The allowlist is
  `cx.tcx.is_diagnostic_item`, except `String` and `Box`, which are
  lang items — `field_copy::borrowed_form` has the same split and
  the comment explaining it.

The rule must **not** call `field_copy::field_copy` to implement its
stand-down exemption from behind. Running the sibling's recogniser
inside this rule couples the two passes and makes the suppression
invisible from the sibling's side. Take the same approach the rest of
the catalogue takes to overlap: state the condition in this rule's own
terms — the return type is a field's type and the body copies that
field — or let both fire and settle it with the ordinary
`#[expect(…)]`, whichever review prefers.

### Difficulty

**Low.** No body analysis, no cross-module state, no source
re-parsing: the pass reads a signature. `owns_allocation` is the
only part with a decision in it, and the decision is which list to
write, not how to compute anything.

The cost is in the design, not the code — which is why this file
exists before any of it.
