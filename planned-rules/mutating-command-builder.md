# `mutating_command_builder`

**Sources:** this repository's own use of
[`command-extra`](https://crates.io/crates/command-extra), and the
review thread on
<https://github.com/KSXGitHub/perfectionist/pull/450> that turned a
`Command::env_remove` call into `CommandExtra::without_env`. No
upstream style guide prescribes this; the policy is the one this
project already follows everywhere it builds a subprocess.

## Status

Implemented in
[`src/rules/mutating_command_builder.rs`](../src/rules/mutating_command_builder.rs),
apart from three of the setters in the table below.

Not implemented: **`stdin`, `stdout` and `stderr`.** Theirs is the one
pair in the table that is not signature-equivalent — std takes anything
`Into<Stdio>` where `CommandExtra` takes a concrete `Stdio` — so the
rename is `E0308` for the `File` and `ChildStdout` arguments that are
the common idiom. Firing only on an argument already of type `Stdio`
would cover the rest, and needs a way to recognise `Stdio`, which
carries no `rustc_diagnostic_item`; this crate identifies types only by
those. Deferred rather than retracted: the `Stdio`-argument case is
worth linting and nothing else covers it.

Three claims in this file did not survive the implementation:

- **The prescribed receiver check does not implement its own
  exemption.** The exemption names a field reached through `&mut self`,
  and rightly; the type comparison the implementation notes prescribe
  does not catch it, since such a field has type `Command` with no
  reference to reject. Moving out of it is `E0507`. The pass walks the
  receiver's place expression instead, and also declines a field of a
  `Drop` type (`E0509`) and a binding captured by a closure (`E0507`).
- **Shape 1 is the wrong conservative subset.** The `Avoid` example
  below is shape 2 — a statement over a `mut` binding — so a
  shape-1-only first pass would have missed the case the rule exists
  for. Both shapes fire.
- **A rename is machine-applicable only under conditions this file does
  not state.** `CommandExtra` has to be in scope in the calling module,
  the receiver has to be a temporary rather than a place the caller
  still holds, the parent has to be one of `Command`'s own methods, and
  there must be no turbofish and no macro expansion. Each omission was
  an unsound rewrite that `cargo fix` reverted the whole file over.

## Statement

`std::process::Command`'s setters take `&mut self` and return
`&mut Command`. They chain, but the chain cannot produce a value, so a
command that needs several settings has to be built over a mutable
binding and handed back separately. `command_extra::CommandExtra`
provides the same settings taking `self` and returning `Self`, which
puts the whole construction in expression position: it can be
returned, bound, stored in a field, or folded over.

Where the `Command` is owned, prefer the `CommandExtra` form.

**Avoid** — the function cannot end in its chain, because the chain
has type `&mut Command`:

```rust
fn lister(dir: &Path) -> Command {
    let mut command = Command::new("ls");
    command
        .current_dir(dir)
        .args(["-l", "-a"])
        .env("LANG", "C");
    command
}
```

**Prefer** — one expression, no binding:

```rust
fn lister(dir: &Path) -> Command {
    Command::new("ls")
        .with_current_dir(dir)
        .with_args(["-l", "-a"])
        .with_env("LANG", "C")
}
```

## Why restrict this?

This is a stylistic preference, not a correctness issue. Both forms
build the same command, and std's form is not wrong.

The preference is that a builder whose steps each yield a value
composes with everything else, and one whose steps yield a borrow
composes with nothing. The `&mut` form cannot be returned from a
function, cannot initialise a `let` in one expression, cannot fill a
struct field, and cannot be an accumulator. Each of those forces the
author to introduce a `mut` binding whose only job is to exist until
the settings are done. The by-value form removes that intermediate,
and the project prefers reading a subprocess definition as a single
expression.

## What to lint

A method call on a receiver whose type is `std::process::Command` —
owned, not a reference — naming one of the setters below.

| std setter    | `CommandExtra` form |
|---------------|---------------------|
| `arg`         | `with_arg`          |
| `args`        | `with_args`         |
| `env`         | `with_env`          |
| `envs`        | `with_envs`         |
| `env_remove`  | `without_env`       |
| `env_clear`   | `with_no_env`       |
| `current_dir` | `with_current_dir`  |
| `stdin`       | `with_stdin`        |
| `stdout`      | `with_stdout`       |
| `stderr`      | `with_stderr`       |

`Command::new` is not a setter and is not flagged. Neither are the
spawning methods (`spawn`, `output`, `status`), which have no
`CommandExtra` counterpart and legitimately take `&mut self`.

### Exemptions

- **A receiver of type `&mut Command`.** This is the exemption that
  matters, and getting it wrong makes the lint unfixable rather than
  merely noisy. `CommandExtra`'s methods take `self`, so code holding
  a borrow — a `fn configure(command: &mut Command)`, a closure
  parameter, a struct field accessed through `&mut self` — *cannot*
  adopt them. Firing there would emit a diagnostic with no valid fix.
  The trigger must confirm the receiver is owned.
- **A crate that does not depend on `command-extra`.** Suggesting a
  method from a crate the author does not have is a suggestion they
  cannot apply. Gated by `require_command_extra_dependency` below.
- **Macro-synthesised calls**, per
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).

Test code is *not* exempt. The preference is about how a command
reads, which does not change between a test and a binary, and this
repository's own test-support helpers are among the code that follows
it.

## Configuration

```toml
["perfectionist::mutating_command_builder"]
# Whether to stay silent in a crate whose dependency graph does not
# include `command-extra`. Defaults to `true`: without the
# dependency the suggested method does not exist, so the diagnostic
# would name something the author cannot write. Set to `false` in a
# workspace that adds the dependency per-crate and wants the lint to
# say so.
require_command_extra_dependency = true
```

## Implementation notes

A `LateLintPass` over expressions. For each method call, ask
`cx.typeck_results()` for the receiver's type and compare it against
`std::process::Command` by `DefId`. Requiring the type itself rather
than a reference to it is necessary but not sufficient — see the
place-expression walk in the Status section above. The setter set is a
fixed table, so resolution is a name match against that table once the
receiver type matches — no trait resolution needed, because these are
inherent methods on `Command`.

The suggestion is not always a rename. The shapes to distinguish:

1. **The call is already an expression whose value is used** — a
   chain, or a tail expression. Renaming the method is the whole fix
   wherever the context accepts a `Command` in place of the
   `&mut Command` the original yielded, which is not everywhere; the
   Status section has the counter-example.
2. **The call is a statement on a `mut` binding** (`command.arg(x);`).
   The by-value form needs `command = command.with_arg(x);`, or the
   binding collapsing into a single chained expression. The second is
   what the rule actually wants and it is not a local rewrite: it
   depends on what else the body does with the binding. Emit advice
   here rather than a machine-applicable suggestion.

Both shapes are implemented, as advice. Covering only shape 1 would
have left the case this rule exists for unflagged: a chain spilled
into a binding because std's setters return a borrow *is* shape 2.

### Difficulty

**Medium.** The trigger is local to one expression and needs no
cross-function reasoning, which keeps it well inside what the existing
late passes do. The parts that need care are the owned-versus-borrowed
receiver check, where a false positive is unfixable rather than
cosmetic, and the statement-shaped case above, where the honest answer
is to suggest less than the rule would like.

### Default state

Active by default.

`require_command_extra_dependency` is a trigger condition rather than a
third state, and it is what makes active-by-default defensible: at its
default the lint cannot fire in a crate that has not already chosen
`command-extra`, so it never pushes a third-party dependency on anyone.
Where the crate has chosen it, the project's position is that the choice
should be followed consistently rather than per call site.

## Interaction with sibling rules

`folded_command_setter` ([`folded-command-setter.md`](./folded-command-setter.md))
flags a *fold* over a singular `CommandExtra` setter where the plural
exists. The two compose rather than overlap:

```rust
items.iter().fold(command, |mut c, a| { c.arg(a); c })
```

Only this rule fires here. The sibling's trigger needs a `CommandExtra`
setter and a closure that forwards, and this shape has neither — the
folder is `Command::arg`, and the body is a block. This rule sees the
`c.arg(a)` inside and advises the by-value form; taking that advice
collapses the closure to `|c, a| c.with_arg(a)`, and the sibling then
fires on *that* and suggests `command.with_args(items)`.

So this rule must not stand down inside a fold. It is the only rule
that reaches the `c.arg(a)`, and the sequence does not start until it
has.

`perfectionist::needless_utf8_conversion`
([`needless-utf8-conversion.md`](./needless-utf8-conversion.md))
already treats the `CommandExtra` setters as recognised
`AsRef<OsStr>` sinks. A crate that adopts this rule therefore gains
that rule's coverage on the same call sites, which is an argument for
the two landing in either order without coordination.

`perfectionist::pipe_style` ([`pipe-style.md`](./pipe-style.md)) is
the closest rule in kind: a policy about an idiom from one of this
author's helper crates rather than about a language construct. It is
worth reading for the shape of its "What to lint" section, which
carries the same burden of saying precisely when a preferred form is
applicable.
