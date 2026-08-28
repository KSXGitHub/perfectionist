# `needless_option_parameter`

**Source:** [KSXGitHub/perfectionist#309](https://github.com/KSXGitHub/perfectionist/issues/309),
generalised from `Result` to `Option`. The
[`needless_result_parameter`](./needless-result-parameter.md) rule is
the issue's direct subject; this rule applies the same reasoning to a
parameter typed `Option<T>`.

## Statement

When a function parameter is typed `Option<T>` but every use of that
parameter in the body immediately reduces it to its `Some` value — by
panicking (`unwrap`, `expect`, `unwrap_unchecked`) or by propagating
(`?`) — the function never handles the `None` case. It only demands
presence. The `Option` in the signature claims to accept an
absent-or-present `T` while the body requires a present one and merely
defers the panic / propagation to a fixed point inside itself. The
`Option` wrapper on the parameter is needless; the function should
take `T`.

As with [`needless_result_parameter`](./needless-result-parameter.md),
the lint is named for the *needless wrapper*, not for any single way of
stripping it, because the trigger fires on the panicking forms and the
`?` form alike.

**Avoid:** the parameter is an `Option`, but the function's only
relationship with the empty case is to panic on it.

```rust
fn render(template: Option<Template>, ctx: &Context) -> String {
    template.expect("template was not loaded").render(ctx)
}
```

**Prefer:** take the `Some` type directly. The `expect` moves to the
call site, where the caller decides what an absent template means.

```rust
fn render(template: Template, ctx: &Context) -> String {
    template.render(ctx)
}
```

## Why restrict this?

This is a stylistic preference, not a correctness issue — the
`unwrap`/`expect`/`?` code behaves identically to the preferred form.
The argument is the same as for
[`needless_result_parameter`](./needless-result-parameter.md): the
signature misrepresents the contract (`Option<T>` promises to cope with
absence; a body opening with `.unwrap()` copes with nothing), it strips
the caller's choice about what `None` means, and it misplaces the panic
one frame away from the code that owns the policy.

`Option` carries a weaker version of the argument than `Result`,
though, which is why this rule is a separate, opt-in lint rather than
part of the `Result` one — see [Default state](#default-state).

## What to lint

Identical predicate to
[`needless_result_parameter`](./needless-result-parameter.md), with
`Option<T>` in place of `Result<T, E>` and `None` in place of `Err`.
For each function parameter `p` typed `Option<T>` (the standard
`core::option::Option`), fire when **every** use of `p` is a
presence-demanding operation —

- `p.unwrap()`
- `p.expect(<msg>)`
- `p.unwrap_unchecked()`
- `p?` (the `?` operator)

— and `p` is used in no other way. Any operation that inspects or
handles `None` (`match` / `if let`, `p.unwrap_or(..)`,
`p.unwrap_or_else(..)`, `p.unwrap_or_default()`, `p.ok_or(..)`,
`p.map(..)`, `p.is_some()`, `p.take()`, passing `p` onward as an
`Option`, …) disqualifies the parameter.

The conservative starting cut (fire only on an exactly-once use), the
trait-method and referenced-as-a-value exemptions, the
closures-out-of-scope scope, and the proc-macro suppression guard all
carry over verbatim from the `Result` rule; see
[its "What to lint" section](./needless-result-parameter.md#what-to-lint).
The two rules share the "parameter whose only uses are
success-demanding" body walk — factor it into a crate-internal helper
when implementing whichever lands first, parameterised by the wrapper
type (`Result` vs `Option`) and the propagation desugaring.

## Examples

**Avoid:** propagation form — the parameter is `?`-ed into the
function's own `Option` return.

```rust
fn first_word(line: Option<String>) -> Option<String> {
    let text = line?;
    text.split_whitespace().next().map(str::to_owned)
}
```

**Prefer:** take the `Some` type; the caller writes `first_word(line?)`.

```rust
fn first_word(text: String) -> Option<String> {
    text.split_whitespace().next().map(str::to_owned)
}
```

**Not flagged:** the `None` case is genuinely handled.

```rust
fn render(template: Option<Template>, ctx: &Context) -> String {
    match template {
        Some(t) => t.render(ctx),
        None => String::new(),
    }
}
```

## Configuration

```toml
["perfectionist::needless_option_parameter"]
# Method calls that count as "demand the Some value or panic".
panic_methods = ["unwrap", "expect", "unwrap_unchecked"]

# Whether the `?` operator counts as a presence-demanding use.
include_question_mark = true
```

## Implementation notes

See [`needless_result_parameter`](./needless-result-parameter.md)'s
implementation notes; the only differences are the parameter-type test
(`Option<_>` rather than `Result<_, _>`) and the `?`-desugaring shape
(`Option`'s `?` lowers through a different `Try` branch). `LateLintPass`,
same visitor, same `MaybeIncorrect` signature-only autofix.

### Difficulty

**Medium**, same as the `Result` rule — and cheaper once that rule
exists, because the shared helper does the heavy lifting and this rule
supplies only the type predicate.

## Default state

Inactive by default. Unlike the `Result` case, taking `Option<T>` and
unwrapping it is *frequently* idiomatic and not a smell: arguments
threaded from `HashMap::get`, builder fields that default when absent,
and `Option`-returning APIs whose `None` truly is a programmer error at
a given call site all produce a legitimate `param.unwrap()`. Shipping
this active by default would false-positive on ordinary code, which
the [rule activation model](./IMPLEMENTATION_CONVENTIONS.md#rule-activation-model)
reserves `Inactive by default` for. A project
that wants the stricter `Option` discipline opts in via
`[perfectionist].enable`.

## Interaction with sibling lints

- [`needless_result_parameter`](./needless-result-parameter.md) is the
  `Result<T, E>` counterpart and the issue's primary subject. It ships
  active by default; this `Option` rule is its opt-in generalisation.
  The two are separate rules — rather than one rule over both wrapper
  types — because their default states differ and because a single name
  cannot honestly describe both triggers.
- [`needless_borrowed_parameters`](./needless-borrowed-parameters.md)
  shares the `needless_*_parameter(s)` naming family: a parameter whose
  wrapper (a borrow, an `Option`, a `Result`) is needless because the
  body only strips it.
- `clippy::unnecessary_wraps` is the return-side analogue for both
  wrappers (a function that returns `Option`/`Result` but never yields
  `None`/`Err`).

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
