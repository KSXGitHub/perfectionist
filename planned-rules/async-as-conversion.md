# `async_as_conversion`

**Sources:** this repository's own review history.
<https://github.com/KSXGitHub/perfectionist/pull/462#discussion_r4071240413>
asked, of `owned_as_conversion`'s exemption for opaque return types:

> Is it even valid to name an `async fn` with an `as_` prefix?
>
> If it's valid, explain.
>
> If it's not valid, if it's an anti-pattern, should detecting and
> flagging this anti-pattern part of this rule or another?

It is not valid, and it is another rule. This file is that rule.

> [!NOTE]
> The sibling this file reasons against, `owned_as_conversion`, is
> still in flight in
> <https://github.com/KSXGitHub/perfectionist/pull/462>, as are
> `borrowed_to_conversion` and `unconsumed_into_conversion` in the two
> PRs stacked on it. None of the three is on `master` yet. Nothing
> here should be implemented before the first of them lands, since the
> argument for this being a separate rule is an argument about what
> that one already covers.

## Statement

`as_` promises a *free* conversion: a borrowed view of something the
receiver already holds. `async` says the opposite about the same
method: this operation may suspend, and nothing happens until an
executor drives it to completion.

A view of data you already hold cannot need suspending. So an
`async fn as_*` names an action as if it were a free conversion,
whatever it returns.

**Avoid:**

```rust
impl Session {
    async fn as_token(&self) -> String {
        self.refresh().await
    }
}
```

**Prefer:** a verb, since what it does is fetch rather than view

```rust
impl Session {
    async fn fetch_token(&self) -> String {
        self.refresh().await
    }
}
```

## Only `as_`, and why

The obvious generalisation — every conversion prefix disagrees with
`async` — is wrong, and the corpus below contains the counterexample
that shows it.

`as_` is the only one of the three whose promise `async` contradicts.
It promises *free*, and nothing that may suspend is free. The other
two promise something `async` says nothing about:

- `to_` promises the conversion **costs** something. Suspension is a
  kind of cost, so the two agree rather than clash.
- `into_` promises the conversion **consumes** the receiver. That is
  orthogonal to suspension entirely.

`fs-err`'s wrapper is the case to keep in mind:

```rust
/// Destructures `File` into a [`fs_err::File`]. This function is async
/// to allow any in-flight operations to complete.
pub async fn into_std(self) -> crate::File { /* ... */ }
```

The name is right, the `async` is honest, and a rule spanning all
three prefixes would report it. So the trigger stops at `as_`.

## Why restrict this?

This is a stylistic preference, not a correctness issue. The method
compiles and returns the right value.

The preference is that `as_` is a cost signal, and a caller who reads
it budgets nothing — the same reason `perfectionist::owned_as_conversion`
exists. `async` occupies that slot with a stronger claim, and the
signature wins over the name, so the prefix is worse than useless: it
is a claim the reader must learn to disregard.

The rename is the whole fix. Nothing about the body changes.

## How often is this written?

A count over the local crate registry cache, 379 crates: **162**
`async fn` declarations, of which **zero** are named `as_*`. The
commonest `async fn` name openings are `write`, `read`, `acquire`,
`send` and `create` — verbs, every one. The single conversion-prefixed
hit in the whole sample is the `into_std` above, which this rule does
not cover.

> [!IMPORTANT]
> Read that the same way as the sibling rule's count. It says the
> convention is real and essentially never violated in published
> libraries, and it says the rule will rarely fire there. The
> violations belong to application code and to generated code, which
> is what this catalogue is for. If that does not convince, close this
> file rather than implement it.

## What to lint

A `LateLintPass` over an inherent method where both hold:

1. **The name begins with `as_`**, a proper prefix — the test
   `owned_as_conversion` applies, which admits `as_name` and rejects
   `ascii_name`.
2. **The method is `async`**, which is `cx.tcx.asyncness(def_id)`.

Nothing about the receiver, the body or the return type is read. An
`async fn` under this prefix is the violation on its own.

### Exemptions

- **A trait impl's method**, because the trait fixes the signature.
- **Macro-synthesised methods**, per
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).

`crate::field_copy::eligible_method` holds both tests already, but it
also demands a `&self` receiver and no other parameter. That costs
this rule nothing — an `as_*` taking anything else is already a
different mistake — so it can be reused as it stands.

## Interaction with sibling rules

`perfectionist::owned_as_conversion` holds the same prefix to the same
promise, and the two cannot both fire. That rule reads a return type it
can name as owning; what an `async fn` signature names is the future,
which is on no such list. So `async fn as_token(&self) -> String` is
reported here and by nothing else, and the exemption that rule
documents for opaque return types is what leaves the case free.

The fixes differ, which is the argument for two rules rather than one.
`owned_as_conversion` sends an `as_*` to `to_*`. This rule cannot:
`to_` promises a pure conversion as much as `as_` does, so the rename
has to reach a verb. A single rule emitting two unrelated remedies
under one name would also have to claim both triggers in that name,
and `owned_as_conversion` claims exactly the one it checks.

`perfectionist::borrowed_to_conversion` and
`perfectionist::unconsumed_into_conversion` hold the other two
prefixes. Neither reads asyncness, and per
[Only `as_`, and why](#only-as_-and-why) neither should.

## Configuration

None. `as_` gets its meaning from the API Guidelines rather than from
a project's taste, and a project that disagrees wants the rule off
rather than tuned.

## Default state

Active by default. The trigger reads two properties of a signature and
nothing else, and the count above puts its false-positive rate on real
published code at zero out of 162.

## Autofix

None. The suggestion is a rename, which breaks every caller, including
ones outside the crate being linted. There is also no single right
replacement: `fetch_`, `load_`, `read_` and `request_` all beat the
prefix, and which one fits is the author's call. Name the problem and
let a human choose.

## Implementation notes

The smallest rule in the catalogue.

- The prefix test is `owned_as_conversion`'s `AS_PREFIX` and the
  `starts_with` beside it. Two rules wanting it is the point at which
  it moves to `crate::common`.
- `cx.tcx.asyncness(def_id)` answers condition 2 directly.
- The guards are `crate::field_copy::eligible_method`, unchanged.

### Difficulty

**Low.** No body analysis, no type analysis, no source re-parsing:
two questions about a signature, both already answerable from helpers
the crate has.
