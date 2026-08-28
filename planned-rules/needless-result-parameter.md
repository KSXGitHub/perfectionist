# `needless_result_parameter`

**Source:** [KSXGitHub/perfectionist#309](https://github.com/KSXGitHub/perfectionist/issues/309).
Spotted in AI-generated code: a function takes a `Result` argument
only to `unwrap` / `expect` / `?`-propagate it in the very first
thing it does, so the parameter's `Result`-ness is stripped before
the function does any real work — the wrapper on the parameter is
needless, and the function only ever wanted the inner `Ok` value.

## Statement

When a function parameter is typed `Result<T, E>` but every use of
that parameter in the body immediately reduces it to its `Ok` value —
by panicking (`unwrap`, `expect`, `unwrap_unchecked`) or by
propagating (`?`) — the function never actually *handles* the `Err`
case. It only demands success. The `Result` in the signature is then
a lie: the function does not accept a fallible value, it requires an
`Ok` one and merely defers the panic / propagation to a fixed point
inside its own body. The `Result` wrapper on the parameter is
needless; the function should take `T`.

The lint is named for that structural defect — the *needless wrapper*
— rather than for any one way of stripping it, because the trigger
fires on both the panicking forms and the `?` form. (A name like
`unwrapped_result_parameter` would describe only `unwrap`/`expect` and
mis-scope the `?` case.)

The motivating example from the issue, condensed:

**Avoid:** the parameter is a `Result`, but the function's only
relationship with the error case is to panic on it.

```rust
fn from_selection(
    selection: io::Result<Navigation>,
    value: impl FnOnce(usize) -> Value,
) -> Result<Resolution, Termination> {
    match selection.expect("interactive selection failed") {
        Navigation::Selected(index) => index.pipe(value).pipe(Resolution::Chosen).pipe(Ok),
        Navigation::Back => Ok(Resolution::Back),
        Navigation::Quit => Err(Termination::Cancelled),
    }
}
```

**Prefer:** take the `Ok` type directly. The `expect` moves to the
call site, where the caller already has the context to decide whether
a failed selection should panic, be propagated, or be handled.

```rust
fn from_selection(
    selection: Navigation,
    value: impl FnOnce(usize) -> Value,
) -> Result<Resolution, Termination> {
    match selection {
        Navigation::Selected(index) => index.pipe(value).pipe(Resolution::Chosen).pipe(Ok),
        Navigation::Back => Ok(Resolution::Back),
        Navigation::Quit => Err(Termination::Cancelled),
    }
}
```

A caller that holds a `Result` writes
`from_selection(selection.expect("interactive selection failed"), value)`
for the `unwrap`/`expect` form, or `from_selection(selection?, value)`
for the propagation form — the same operation the wrapped signature
was hiding, now visible at the point that owns the error policy.

## Why restrict this?

This is a stylistic preference, not a correctness issue: the
`unwrap`/`expect`/`?` code compiles and behaves identically to the
preferred form. The objection is to where the fallibility is resolved,
not to whether the program works.

- **The signature misrepresents the contract.** `fn f(r: Result<T, E>)`
  reads as "this function copes with a possibly-failed `T`", but a body
  that opens with `r.unwrap()` copes with nothing — it demands a `T`.
  Taking `T` states the real requirement.
- **It strips the caller's choice.** A `Result` is interesting exactly
  because the holder can decide what to do with an `Err`: panic,
  propagate, retry, substitute a default, log and continue. Burying a
  fixed `unwrap`/`expect`/`?` inside the callee removes that choice from
  every caller and freezes one policy for all of them.
- **It misplaces the panic.** `expect("interactive selection failed")`
  produces a better message and a more useful backtrace when it sits at
  the call site that actually performed the selection than when it sits
  one frame removed inside a helper that never saw the I/O.

This is the parameter-side mirror of `clippy::unnecessary_wraps`, which
flags a function that *returns* `Result` / `Option` but never produces
`Err` / `None` (the wrapper adds nothing): there the needless `Result`
is on the way out, here it is on the way in. The `needless_` name is
chosen to make that parallel — and the parallel with the catalogue's
own [`needless_borrowed_parameters`](./needless-borrowed-parameters.md)
— legible.

## What to lint

For each function whose signature the author controls, inspect every
parameter `p` declared with type `Result<T, E>` (the standard
`core::result::Result`; aliases that resolve to it count). The
parameter is a violation when **every** use of `p` in the body is one
of the *success-demanding* operations:

- `p.unwrap()`
- `p.expect(<msg>)`
- `p.unwrap_unchecked()`
- `p?` (the `?` operator, including the `try!` macro form)

and `p` is used in no other way. An operation that *inspects or
handles* the `Err` disqualifies the parameter — the function is doing
real work with the error, so its `Result` parameter is earned:

- `match` / `if let` on `p`, `p.map_err(..)` not immediately `?`-ed,
  `p.ok()`, `p.is_ok()` / `p.is_err()`, `p.unwrap_or(..)`,
  `p.unwrap_or_else(..)`, `p.unwrap_or_default()`, `p.ok_or(..)`,
  `p.and_then(..)`, passing `p` onward as a `Result`, storing it, etc.

### Conservative starting point

Mirror the first cut of
[`needless_borrowed_parameters`](./needless-borrowed-parameters.md):
fire only when `p` is used **exactly once** and that single use is a
success-demanding operation. This catches the issue's motivating
example (`selection.expect(...)` as the lone use) without the
control-flow analysis that the multi-use case needs. The
dominance-aware version — `p` used several times, every use a
success-demanding op on every path — is a later extension.

### Exemptions

The lint stays silent when:

- **The signature is dictated by a trait.** A trait method declaration
  or a trait `impl` method cannot change its parameter type; skip when
  `tcx.trait_of_item(def_id).is_some()`.
- **The function is referenced as a value**, not only called — passed
  as a `fn` item / function pointer, or `&`-borrowed — anywhere in the
  crate. Its `fn(Result<T, E>) -> _` shape may be load-bearing (e.g.
  handed to a combinator that supplies a `Result`), so the parameter
  type is not freely changeable. (Whole-crate "is it ever used as a
  value?" is itself non-trivial; a conservative implementation may
  approximate it and lean on a local
  `#[expect(perfectionist::needless_result_parameter)]` for the rare
  miss.)
- **The parameter is also used in `Result` form elsewhere** in the
  body — the exactly-once rule already excludes this, but it remains
  the governing principle when the multi-use version lands.
- Proc-macro-synthesised parameters (see the
  [proc-macro suppression convention](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations))
  — guard with `crate::common::hir_in_external_macro` since the
  diagnostic span is the narrow parameter, not the whole node.

Closures are out of scope: their parameter types are usually inferred
rather than written, and the "the signature lies" argument is about a
written type annotation.

## Examples

**Avoid:** propagation form — the parameter is `?`-ed straight into the
function's own `Result` return.

```rust
fn parse_config(raw: Result<String, io::Error>) -> Result<Config, ConfigError> {
    let text = raw?;
    Config::from_str(&text)
}
```

**Prefer:** take the `Ok` type; the caller writes `parse_config(raw?)`.

```rust
fn parse_config(text: String) -> Result<Config, ConfigError> {
    Config::from_str(&text)
}
```

**Not flagged:** the `Err` is genuinely handled — the function does
real work with the failure case.

```rust
fn parse_config(raw: Result<String, io::Error>) -> Config {
    match raw {
        Ok(text) => Config::from_str(&text).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}
```

**Not flagged:** `unwrap_or_else` substitutes a value for the `Err`
rather than demanding `Ok`.

```rust
fn line_count(contents: Result<String, io::Error>) -> usize {
    contents.unwrap_or_else(|_| String::new()).lines().count()
}
```

## Configuration

```toml
["perfectionist::needless_result_parameter"]
# Method calls that count as "demand the Ok value or panic". The
# defaults cover the std inherent methods; extend for project-specific
# wrappers that panic on Err (e.g. a logging `expect`-alike).
panic_methods = ["unwrap", "expect", "unwrap_unchecked"]

# Whether the `?` operator counts as a success-demanding use. The
# issue lists error-propagation explicitly, so this defaults to `true`;
# set it to `false` to restrict the lint to the panicking forms only.
include_question_mark = true
```

The `panic_methods` list follows the repository's extend-the-defaults
convention rather than replacing them; pair it with an
`ignore_panic_methods` list if a suppression knob proves necessary.

## Implementation notes

- `LateLintPass`. Visit every `ItemKind::Fn` and inherent
  `ImplItemKind::Fn`; skip trait-dictated methods.
- For each parameter, resolve its type and test whether it is
  `Result<_, _>`. Record the parameter's `HirId` as a candidate.
- Walk the body with a `Visitor` that finds every `Path` expression
  resolving to the candidate parameter. For each, look at the parent
  expression:
  - a `MethodCall` whose segment is in `panic_methods` → success-demanding;
  - an `ExprKind::Match` synthesised by the `?` desugaring whose
    scrutinee is the parameter (or `clippy_utils`' `?` helpers) →
    success-demanding when `include_question_mark`;
  - anything else → a real use that disqualifies the parameter.
- Fire when the candidate has at least one use and **all** uses are
  success-demanding (conservative cut: exactly one use).
- Suggested fix: rewrite the parameter type from `Result<T, E>` to `T`
  and drop the success-demanding call at the (single) use site.
  `Applicability::MaybeIncorrect` — the call sites live outside the
  linted function and cannot be rewritten by a per-function pass, and
  the `?` form additionally requires each caller to be in a
  `?`-capable context. Emit the signature change as the machine-
  applicable part and describe the call-site follow-up in the note.

### Difficulty

**Medium.** The exactly-once conservative cut is a straightforward HIR
walk: match the parameter type, find its lone use, classify the
enclosing call. The harder pieces are the multi-use dominance analysis
(shared in spirit with `needless_borrowed_parameters`) and the
"referenced as a value" whole-crate exemption, both of which can be
deferred behind the conservative starting point.

## Default state

Active by default. The issue treats the `Result`-parameter case as an
unambiguous anti-pattern, and a `Result` parameter consumed only by
`unwrap`/`expect`/`?` has no legitimate reading that the preferred form
does not express more honestly.

## Interaction with sibling lints

- [`needless_option_parameter`](./needless-option-parameter.md) is
  the `Option<T>` counterpart: a parameter taken only to be `unwrap` /
  `expect` / `?`-ed. It is a separate rule because `Option` unwrapping
  is far more often idiomatic (builder defaults, `take()` plumbing), so
  it ships **inactive by default** while this `Result` rule is active —
  a default-state split that one rule cannot express, and one that also
  keeps each rule's name accurate to the single type it fires on. The
  two rules share the "parameter whose only uses are success-demanding"
  walk; factor that into a crate-internal helper when implementing
  whichever lands first.
- [`needless_borrowed_parameters`](./needless-borrowed-parameters.md)
  is the catalogue's other "the wrapper on the parameter is needless"
  rule (there, a `&T` whose body only `.to_owned()`s it). Same naming
  family, same conservative exactly-once starting cut.
- `clippy::unnecessary_wraps` is the return-side analogue (a function
  that wraps its result in `Result`/`Option` but never yields
  `Err`/`None`). Enabling both covers needless fallibility on both ends
  of a signature.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
