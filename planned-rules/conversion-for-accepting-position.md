# `conversion_for_accepting_position`

**Source:** this repository's own review history.
<https://github.com/KSXGitHub/perfectionist/pull/586#discussion_r4430603983>
asked, of a suggestion that had just been rewritten from
`os_str.to_str() == Some("x")` to `os_str == OsStr::new("x")`:

> If one write the following lines:
>
> ```rust
> let a = os_str == OsStr::new("x");
> let b = os_str == "x".as_ref();
> let c = os_str == OsStr::new(str_slice);
> let d = os_str == str_slice.as_ref();
> let c = os_str == path.as_os_str();
> let d = os_str == path.as_ref();
> ```
>
> Would any of the lines trigger any rustc or Clippy lint?
>
> Since, y'know, the intermediary steps are redundant.

The reply in
<https://github.com/KSXGitHub/perfectionist/pull/586#discussion_r4430611410>
established that nothing fires: every rustc lint and the whole of
`clippy::all`, `pedantic`, `nursery`, `cargo` and `restriction` left
the comparisons alone, and the nearest lint,
`clippy::cmp_owned`, is priced on the **allocation** rather than on
the redundancy. `OsStr::new` allocates nothing, so there is nothing
for it to object to.

That gap is this rule.

## Statement

A value is converted so that it will fit the position it is about to
stand in, and that position already accepted it unconverted. The
conversion changes the type, satisfies nothing that was not already
satisfied, and the reader has to work out which of those two it was
doing.

The shape review asked about:

**Avoid.** The literal is wrapped in the type of the thing it is
compared against:

```rust
os_str == OsStr::new("x")
```

**Prefer.** `OsStr` has compared equal to `str` since Rust 1.0.0, so
the wrapper names a type the comparison already knew:

```rust
os_str == "x"
```

The same redundancy turns up in places other than a comparison.

**Avoid.** A parameter bounded by `AsRef<Path>` accepts a `&str`, so
wrapping the argument in `Path::new` hands the callee something it
would have built itself:

```rust
File::open(Path::new("config.toml"))
```

**Prefer:**

```rust
File::open("config.toml")
```

What the comparison and the call have in common is not the operator
and not the type. It is that the position had already said what it
accepts, and the author converted anyway.

## Why restrict this?

This is a stylistic preference, not a correctness issue. For every
form the rule fires on, the two spellings compute the same answer on
every input, which
[The rewrite is an identity](#the-rewrite-is-an-identity) establishes
family by family. The objection is to what the reader has to carry:

- **A type named for no reason.** `OsStr::new("x")` tells the reader
  that an `OsStr` is wanted here. The comparison already told them
  that, by having an `OsStr` as its other operand. The second telling
  is not a second fact.
- **A question the reader has to answer to rule the conversion out.**
  A conversion inside a comparison sometimes cannot be dropped
  without changing what the comparison does, and
  [(e)](#e-the-rewrite-must-select-an-impl-that-agrees) records the
  cases in this very neighbourhood where that is so. So a reader who
  meets one cannot skip it: they have to decide whether it changes
  which `PartialEq` impl is selected. Where the answer is no, the
  conversion cost them that decision and bought nothing.
- **A claim about reachability that was never true.** The wrapper
  reads as "this would not compile without me". For the OS-string
  family that has been false since Rust 1.0.0, which is the whole
  life of the stable language.

### Cost is not the argument

The conversions this rule reports are free. `OsStr::new` is a
reborrow, `Path::new` is a reborrow, and `as_str` / `as_slice` /
`as_os_str` / `as_path` hand back a view of storage that already
exists. Removing one saves no allocation and no work, and the rule
does not claim otherwise.

That is deliberate, and it is what separates this rule from the
lints that already cover part of this neighbourhood.
`clippy::cmp_owned` and `clippy::unnecessary_to_owned` fire on the
*allocating* conversions in these same positions, and they fire
because of the allocation;
[Interaction with Clippy and rustc](#interaction-with-clippy-and-rustc)
shows which conversions each one recognises and which it walks past.
A rule motivated by cost would be a worse version of those two. A
rule motivated by wording reaches the ones they decline, and reaches
them for a reason that does not need a benchmark to defend.

### The rewrite is an identity

Each family this rule covers was checked on this host, by compiling
both spellings and comparing their answers rather than by reading the
impls.

**The OS-string family.** `impl PartialEq<str> for OsStr` and
`impl PartialEq<str> for OsString` are both `#[stable(feature =
"rust1", since = "1.0.0")]`, and both compare the encoded bytes. The
one level std added later is `impl PartialEq<&str> for OsString`,
which is `#[stable(feature = "os_str_str_ref_eq", since = "1.29.0")]`
and is what an owned `OsString` compared against a literal needs.
`os_str == lit` agreed with `os_str == OsStr::new(lit)` on every one
of 67072 comparisons: each of the 256 byte values as a one-byte OS
string against a spread of literals, and every two-byte pair against
a literal derived from its first byte. `OsStr` also has
`PartialOrd<str>`, so the ordering operators belong to this family
too.

**The byte-buffer and text families.** `Vec<u8>` compares against a
byte-string literal and `String` against a string literal, so
`v.as_slice() == b"xy"` and `s.as_str() == "x"` each drop their view.
`v == lit` agreed with `v.as_slice() == lit` on all 655360
comparisons: every one-byte and two-byte buffer against literals
including the empty one and two that are not valid UTF-8. `s == lit`
agreed with `s.as_str() == lit` on all 98304 comparisons, over every
`char` below U+2000 against literals including `é` and U+FFFD, and a
`Cow<'_, str>` and a `Box<str>` over the same inputs agreed on 98304
more.

**The path family is the exception, and it is exempt by default.**
`impl PartialEq<str> for Path` exists, so `path == "a/b"` compiles —
but it is `#[stable(feature = "eq_str_for_path", since = "1.91.0")]`,
and it does **not** compare the string. It compares components:
`Path::new("a/b") == "a//b"` is `true`, where `"a/b" == "a//b"` is
`false`. The rewrite is still an identity — `path == lit` agreed with
`path == Path::new(lit)` and with `path == OsStr::new(lit)` on all
242 pairs drawn from a set of shapes including `//`, `a/b/`, `./a`,
`a/./b` and `a/../b` — so the rule could fire here and be correct.
[Configuration](#configuration) says why it does not.

## What to lint

Fire on **(a)** a conversion standing in **(b)** a position that
**(c)** accepts the conversion's own operand unconverted, where
**(d)** the conversion carries nothing else and **(e)** the impl the
rewrite selects agrees with the impl it replaces.

### (a) The conversion

The conversion is one of these, recognised by what it does rather
than by where it sits:

- **A borrowing constructor** whose argument is the converted value:
  `OsStr::new(v)`, `Path::new(v)`. Both take `&S` for
  `S: AsRef<OsStr> + ?Sized` — the same bound, even for the one
  returning `&Path` — and hand back a reborrow.
- **A view method** taking `&self` and handing back a borrow of the
  receiver's own storage: `as_str`, `as_slice`, `as_os_str`,
  `as_path`, `as_bytes`, `as_encoded_bytes`, `as_ref`, `deref`.

The allocating conversions are **not** in the set.
`PathBuf::from(v)`, `v.to_owned()`, `v.to_vec()`, `v.to_path_buf()`
and the rest belong to the lints named in
[Interaction with Clippy and rustc](#interaction-with-clippy-and-rustc),
and the fidelity-destroying UTF-8 ones belong to
`perfectionist::needless_utf8_conversion`. Keeping them out is what
makes [The rewrite is an identity](#the-rewrite-is-an-identity) a
statement about the whole trigger rather than about most of it.

### (b) The position

- **A comparison operand.** `==`, `!=`, `<`, `<=`, `>`, `>=`. Either
  operand may hold the conversion.
- **A call or method argument** whose parameter is generic with a
  single trait bound.

Deliberately not positions, each for a reason worth recording:

- **A `let` with a type annotation.** `let s: &str = string.as_str()`
  could drop its view, because `&string` coerces. The rule declines
  it: an annotation is where a reader goes *looking* for the type, so
  a conversion that names the same type is answering the question the
  annotation asked rather than restating one the reader already had.
  `clippy::explicit_deref_methods` reaches the `deref()` spelling of
  this position and no other spelling of it, which is the same
  judgement drawn narrowly.
- **A `return` or tail expression.** No coercion runs from `&str` to
  `&Path`, so wherever the return type differs from the operand's
  type, dropping the conversion stops the function compiling.
  Checked: `fn f(s: &str) -> &Path { s }` is `E0308`.
- **A struct-literal field.** The field's type is concrete, and
  nothing coerces into it, so a conversion that reaches it is the
  only reason the field type-checks.

### (c) The position has to accept the operand unconverted

Asked of the compiler rather than of a table, so a user type reaches
the rule on the same terms as a std one.

For a comparison, the question is whether the operator's own trait is
implemented between the two operand types **as written**, which means
at the reference depth they are written at. That depth decides it,
and std does not supply every level:

| Written | Compiles |
|---|---|
| `&PathBuf == &str` | yes, through the blanket `impl PartialEq<&B> for &A` |
| `PathBuf == &str` | **no** — `PartialEq<str> for PathBuf` exists, `PartialEq<&str>` does not |
| `OsString == &str` | yes — `PartialEq<&str> for OsString` is supplied |

So a rule that strips the conversion and asks about the base types
reports a rewrite that does not build. It has to ask about the types
the operands have.

`PartialEq` holding is also no evidence that `PartialOrd` does, and
the two shortfalls do not even report alike. `OsStr` has both against
`str`. `Path` has only `PartialEq`, so `PartialOrd::lt` on a `&Path`
and a `&str` is `E0277`, "can't compare `Path` with `str`". `String`
has only `PartialEq`, and there the same call is `E0308`, because
`String`'s only `PartialOrd` is against itself and `&str` will not
unify with it. So each operator has to be checked against its own
trait, and a rule reading the error text rather than the trait would
miss one of the two.

For an argument, the question is whether the operand's type satisfies
the parameter's bound. `perfectionist::folded_command_setter` already
asks a question of this shape, and
[`src/rules/folded_command_setter/setter.rs`](../src/rules/folded_command_setter/setter.rs)
records the trap found while building it: a bound satisfied through a
reference is not the same bound. Reuse that reasoning rather than
rediscovering it.

### (d) The conversion carries nothing else

A conversion can be doing a second job. Each job here was found by
compiling the rewrite rather than by reasoning about it:

- **It pins a type parameter two arguments share.** For
  `fn pair<P: AsRef<Path>>(a: P, b: P)`, the call
  `pair(Path::new("x"), p)` compiles and `pair("x", p)` is `E0308`,
  because `P` cannot be both.
- **It meets a turbofish.** `one::<PathBuf>("x")` is `E0308` where
  `one(PathBuf::from("x"))` compiles.
- **It decides which impl a second bound uses.** A parameter bounded
  `AsRef<[u8]> + Debug` prints `[120, 121]` for `s.as_bytes()` and
  `"xy"` for `s`. `Hash` diverges the same way: on this host,
  hashing `"xy"`, `b"xy"` as a slice and `Path::new("xy")` gave a
  different value for each. So the single-bound
  condition in [(b)](#b-the-position) is not conservatism. A second
  bound makes the source type observable, and the rewrite stops being
  an identity.
- **It builds the type an enclosing wrapper fixes.** `Option` derives
  its `PartialEq` between two of the same type, so
  `p.file_name() == Some(OsStr::new("x"))` compiles and
  `p.file_name() == Some("x")` is `E0308`. A conversion under `Some`,
  `Ok` or any other constructor is reached through that constructor's
  own type, not through the comparison, and is out of the trigger.

### (e) The rewrite must select an impl that agrees

This is the condition that cannot be delegated to the type system,
and the OS-string-against-path case is the proof.

`os_str == path.as_os_str()` compares **bytes**: both operands are
`OsStr`, so `<OsStr as PartialEq>::eq` runs. Drop the view and
`os_str == path` still compiles, because `impl PartialEq<Path> for
OsStr` exists — and it is one of the pair generated by std's
`impl_cmp_os_str!`, stable since 1.8.0, whose body is
`<Path as PartialEq>::eq(self.as_ref(), other)`. It compares
**components**. Checked on this host: `OsStr::new("a/b") ==
Path::new("a//b")` is `true`, and `OsStr::new("a/b") ==
Path::new("a//b").as_os_str()` is `false`.

So that `.as_os_str()` is what holds the comparison to bytes.
Dropping it still compiles, and the answer changes. A rule that fires
on it is wrong, and no amount of trait querying tells it so, because
both spellings type-check and the traits are the same trait.

The consequence for the design: **candidacy is type-driven,
equivalence is not.** The rule carries a closed set of conversions
whose equivalence has been checked at the pairing of source type and
target type, and fires only inside it. That set is not configurable,
per [Configuration](#configuration).

One narrower form of the same hazard: in a comparison where **both**
operands carry a droppable conversion, dropping either is valid and
dropping both need not be. `p.as_path() == Path::new("x")` admits
`p == Path::new("x")` and admits `p.as_path() == "x"`, and `p == "x"`
is `E0277` for an owned `PathBuf`. So the rule reports at most one
operand of a comparison.

### Exemptions

- A conversion the position needs, per
  [(c)](#c-the-position-has-to-accept-the-operand-unconverted).
- A conversion doing a second job, per
  [(d)](#d-the-conversion-carries-nothing-else).
- A conversion whose removal would change the answer, per
  [(e)](#e-the-rewrite-must-select-an-impl-that-agrees).
- A conversion inside the body of the very `PartialEq` or
  `PartialOrd` impl whose rewrite would dispatch back to it. std is
  full of these: `impl PartialEq for str` is
  `self.as_bytes() == other.as_bytes()`, and
  `impl PartialEq<PathBuf> for str` is `self == other.as_path()`.
  Dropping either view spells the impl in terms of itself. rustc's
  `unconditional_recursion` does catch the result, which makes the
  mistake loud rather than acceptable: a suggestion that trades this
  rule's warning for a rustc one has not improved the code.
- A conversion of a path compared against a literal, unless
  `include_path_comparisons` is set, per
  [Configuration](#configuration).
- Proc-macro-synthesised nodes, per
  [Implementation notes](#implementation-notes).

## Examples

### A comparison operand

Both of the sites this repository was found to have are this shape.

**Avoid.** The anchor search in
[`src/cargo_target.rs`](../src/cargo_target.rs) wraps a literal,
where `name` is already an `&OsStr`:

```rust
components.iter().rposition(|name| *name == OsStr::new("src"))
```

**Prefer.** The other operand is what says an `OsStr` is wanted, so
the literal can stand alone:

```rust
components.iter().rposition(|name| *name == "src")
```

**Avoid.** The temp-directory check in
[`utils/src/temp_dir.rs`](../utils/src/temp_dir.rs) wraps a `&str`
constant:

```rust
temp_dir.file_name().filter(|name| *name == OsStr::new(CHOSEN_NAME))
```

**Prefer:**

```rust
temp_dir.file_name().filter(|name| *name == CHOSEN_NAME)
```

**Avoid.** A view taken of an owned buffer that the comparison would
have taken itself:

```rust
bytes.as_slice() == b"magic"
```

**Prefer:**

```rust
bytes == b"magic"
```

### An argument

**Avoid.** The parameter is bounded `AsRef<Path>`, so it accepts the
literal:

```rust
fs::read_to_string(Path::new("config.toml"))
```

**Prefer:**

```rust
fs::read_to_string("config.toml")
```

**Avoid.** A view handed to a bound that would have taken the same
view:

```rust
command.arg(path.as_os_str())
```

**Prefer.** `Command::arg` takes `impl AsRef<OsStr>`, and
`<&Path as AsRef<OsStr>>::as_ref` is `as_os_str`:

```rust
command.arg(path)
```

### Not flagged

```rust
// Not flagged: the comparison is against a `Path`, whose equality
// compares components rather than bytes, and dropping the wrapper
// would leave that spelled like a string comparison. Set
// `include_path_comparisons` to opt in.
root == Path::new("")

// Not flagged: `Option` compares two of the same type, so the
// wrapper is what makes the operands meet at all.
root.file_name() == Some(OsStr::new("main.rs"))

// Not flagged: both operands are `OsStr` here, so the comparison is
// bytewise. Dropping the view would compare an `OsStr` against a
// `Path`, which std implements by comparing components, so the
// answer would change.
os_str == path.as_os_str()

// Not flagged: an owned `PathBuf` has no `PartialEq<&str>`, so the
// literal cannot stand alone on the right of this comparison.
path_buf == Path::new("x")

// Not flagged: `read_workspace_from` takes a concrete `&Path`, and
// a `&String` does not coerce to one.
read_workspace_from(Path::new(&manifest_dir))

// Not flagged: both arguments instantiate one type parameter, and
// the wrapper is what fixes it.
pair(Path::new("x"), other_path)

// Not flagged: this is the body of `impl PartialEq for str`, so
// dropping the views would define the impl in terms of itself.
self.as_bytes() == other.as_bytes()
```

## Suggested fix

Replace the conversion expression with its operand, and say which
operand in the help text so a reader can see the comparison the fix
leaves behind:

> **warning:** `OsStr::new` converts a value this comparison already
> accepts
>
> **help:** compare against `"src"` directly

### The fix can orphan an import

Measured, by applying both of this repository's fixes and
type-checking the workspace: the
[`utils/src/temp_dir.rs`](../utils/src/temp_dir.rs) fix leaves
`use std::ffi::OsStr;` with no remaining use, which is an
`unused_imports` warning and a build failure under this project's
`-D warnings`. The
[`src/cargo_target.rs`](../src/cargo_target.rs) fix does not, because
that module names `OsStr` elsewhere.

So the conversion's own span is the whole of what a span replacement
can reach, and whether the fix is complete depends on a file-wide
question the span does not see. The first implementation should emit
`Applicability::MaybeIncorrect` and leave the import to the author.
Promoting it means proving the removed path has no other use in the
file, which is a separate piece of work and not a precondition for
shipping the trigger.

## Configuration

```toml
["perfectionist::conversion_for_accepting_position"]
# Also report a path compared against a string literal. Off by
# default, because dropping the wrapper leaves a componentwise
# comparison spelled like a bytewise one.
include_path_comparisons = false
```

The path family is the one case where the wrapper earns its place on
grounds the rule cannot measure. `path == Path::new("a/b")` and
`path == "a/b"` give the same answer on every input, and only the
wrapped spelling warns the reader that the answer is componentwise —
that `"a/b"` will match a stored `"a//b"`. The wrapper does no type
work but real documentary work, which is a trade a project should
make for itself rather than inherit.

Keeping the family off by default also puts the rule's MSRV question
behind the knob. `impl PartialEq<str> for Path` is stable
since 1.91.0, so a project with an older `rust-version` cannot take
this fix; a project that sets `include_path_comparisons` needs the
suggestion gated on `clippy_utils::msrvs::Msrv`, which
[`src/rules/map_or.rs`](../src/rules/map_or.rs) already reads from
`Cargo.toml` for the same purpose. Nothing in the default trigger
is newer than `impl PartialEq<&str> for OsString`, stable since
1.29.0, which is below any `rust-version` a project is likely to be
holding.

No knob extends the conversion set. The rule's claim is that the
rewrite is an identity, and
[(e)](#e-the-rewrite-must-select-an-impl-that-agrees) shows that
claim resting on checked pairings rather than on anything the type
system reports. An `extra_conversions` list would let a project
assert an equivalence the rule cannot verify, which is not a knob —
it is a way of turning the rule into a guess.

## Default state

Active by default, for the comparison position over the conversions
in [(a)](#a-the-conversion).

The conditions are exclusions rather than heuristics: each asks the
compiler a question with a yes-or-no answer, and
[(e)](#e-the-rewrite-must-select-an-impl-that-agrees), which cannot
be asked of the compiler, is answered by the closed conversion set
rather than by inference. A site that passes all of them has a
rewrite that compiles and agrees, which both sites found in this
repository were checked to have.

The argument position ships in the same state once it is built. Its
risk is not higher — the single-bound condition is what holds it to
an identity — but its yield here is zero, which
[Difficulty](#difficulty) accounts for.

## Implementation notes

These notes record what is certain about the rule's *shape*. The
`rustc` / `clippy_utils` calls these notes name are ones this
repository already uses; the rest are left to the implementer to
choose and verify, since none of them has been compiled against
here.

- **`LateLintPass`, not `EarlyLintPass`.** Every condition is a
  question about types, so `typeck_results` and `implements_trait`
  are the whole mechanism. An early pass can see the call and nothing
  that decides it.
- **Triggers are HIR expression shapes, not source text.** A
  comparison is a `Binary`, a conversion is a `Call` or a
  `MethodCall`, so the parser-combinator convention in
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  does not apply. The one piece of text handling is lifting the
  operand's snippet into the help text.
- **The conversion set is this rule's own, and overlaps neither
  sibling's.**
  The UTF-8 conversions that
  `perfectionist::needless_utf8_conversion` and
  `perfectionist::utf8_conversion_for_literal_comparison` share are
  excluded here by [(a)](#a-the-conversion), so there is no list to
  factor out and no shared module to build first.
- **The operand types come from the expression, not from the
  conversion's signature.** The reference-depth table in
  [(c)](#c-the-position-has-to-accept-the-operand-unconverted) is the
  reason: `expr_ty` on each operand, and `implements_trait` for the
  operator's trait against those types, is the check. Peeling
  references before asking turns a reported rewrite into one that
  does not build.
- **Proc-macro suppression.** The natural diagnostic span is the
  conversion sub-expression, which is narrower than the enclosing
  comparison. That is the case the
  [proc-macro suppression convention](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations)
  identifies as uncovered by `report_in_external_macro: false`. Apply
  the late-pass `crate::common::hir_in_external_macro` guard it
  prescribes, and add a
  `ui/conversion_for_accepting_position_proc_macro.rs` fixture only
  if a non-vacuous one can be built: delete the guard and confirm the
  fixture turns red. Otherwise record at the span-selection site why
  it is omitted.
- **A fixture is owed to each exemption that compiles.** The
  exemptions that fail to compile are held by the build; the ones
  that compile and change the answer are held by nothing but a test.
  `os_str == path.as_os_str()`, the `Some`-wrapped wrapper, and the
  `impl PartialEq` body each need a `ui/` case, and each is only real
  if removing the exemption that covers it turns the case red.

### Difficulty

**Medium** for the comparison position, which is the core and the
tier both of this repository's sites are in: a `Binary` whose operand
is a recognised conversion, two `expr_ty` queries, one
`implements_trait` per candidate operator, a lookup in the closed
conversion set, and the exemptions in
[(d)](#d-the-conversion-carries-nothing-else).

**Hard** for the argument position, and the difficulty is not the
bound check. It is everything around it: walking `predicates_of` to
establish that the parameter has exactly one bound, recognising a
turbofish, and recognising a type parameter that another argument
also fixes.
[`src/rules/folded_command_setter/setter.rs`](../src/rules/folded_command_setter/setter.rs)
is the nearest prior art and it is worth reading before starting,
because the `&K: AsRef<OsStr>` trap it records is the same trap in a
different position.

The yield is also worth knowing before budgeting for it. Scanning
this repository found no instance: every path-taking and
OS-string-taking parameter here is a concrete `&Path` or `&OsStr`,
which no literal coerces to, so dropping the wrapper at any of those
call sites would stop the call compiling. The tier is in the design
because the redundancy is real where the generic bound is, not
because this repository is waiting on it.

**Deferrable**, and not in the design:

- The `let`-with-annotation position, per
  [(b)](#b-the-position).
- A conversion bound to a local before the position that consumes it.
  Establishing that the binding reaches only accepting positions is
  the local-dataflow problem, and the same deferral is recorded in
  [`utf8-conversion-for-literal-comparison.md`](./utf8-conversion-for-literal-comparison.md)
  for the same reason.
- Promoting the fix to machine-applicable, which needs the
  orphaned-import question in
  [The fix can orphan an import](#the-fix-can-orphan-an-import)
  answered.

## Interaction with Clippy and rustc

**Neither has a lint for this.** Checked against
`clippy 0.1.98 (14cae68132 2026-07-08)` by running the shapes in
[Examples](#examples) under `clippy::all`, `clippy::pedantic`,
`clippy::nursery`, `clippy::cargo` and `clippy::restriction`
together. Nothing fired on any conversion this rule reports. The only
diagnostics were about the probes' own unrelated style
(`clippy::needless_pass_by_value`, `clippy::ptr_arg`,
`clippy::inline_trait_bounds` and others of that kind).

The lints that come closest, each confirmed by running it:

- **`clippy::cmp_owned`** flags an owned value created for a
  comparison. It recognises `String::from`, `to_owned`, `to_string`,
  `OsString::from`, `PathBuf::from` and `Vec::from`, and suggests the
  bare literal for each. It fires on none of this rule's
  conversions, and it also walks past `to_vec`, `to_path_buf`,
  `to_os_string` and `clone` in the same position. Those allocate, so
  [(a)](#a-the-conversion) excludes them here, and `cmp_owned` does
  not reach them: between the two rules they are reported by neither.
- **`clippy::unnecessary_to_owned`** is the argument-position
  counterpart, and sorts the same way: it fires on
  `generic_str(s.to_string())` and on `command.arg(s.to_string())`,
  and not on `generic_osstr(OsStr::new(s))` or
  `generic_path(PathBuf::from("x"))`.
- **`clippy::useless_conversion`** flags `Into::into` and
  `From::from` where the type before and after are **the same**. It
  fires on `String::from(s)` for an owned `String` and not on
  `v == OsStr::new("x")`. So this rule is a widening of it rather
  than a refinement, which is why
  [the name-mirroring convention](./IMPLEMENTATION_CONVENTIONS.md#mirror-the-clippy-name-only-for-a-genuine-refinement)
  forbids borrowing its name: a reader who transferred their
  knowledge of `useless_conversion` would expect the type before the
  conversion and the type after it to be the same, where this rule's
  interest begins once they differ.
- **`clippy::useless_asref`** flags `as_ref` where the type is
  unchanged, which is the identity case again rather than this one.
- **`clippy::explicit_deref_methods`** flags `.deref()` and suggests
  `&*`. It reaches the `let`-annotation position for that one method
  name, and [(b)](#b-the-position) declines that position for every
  method name, `deref` included.
- **`clippy::op_ref`** flags a reference taken of an operand. It
  fires on `v == &OsString::from("x")` and objects to the `&`, not to
  the conversion under it.
- **`unconditional_recursion`**, a rustc built-in, catches the
  mistake the `impl PartialEq` exemption exists to avoid, which makes
  that exemption about not emitting a bad suggestion rather than
  about safety.

## Interaction with sibling rules

- **`perfectionist::needless_utf8_conversion`** reaches the same
  argument position with a different conversion set and a different
  rationale. Its set is the conversions that **lose fidelity** —
  `to_string_lossy()`, `display().to_string()`, the
  `unwrap`-on-`None` forms — and its rewrite is a correction: a
  corrupted path reaching the OS names the wrong file. Its
  heading is "Why is this bad?" for that reason.

  This rule's set is the conversions that are **free and faithful**,
  and its rewrite is an identity, which is what makes its heading
  "Why restrict this?". The two sets are disjoint by
  [(a)](#a-the-conversion), and disjoint in the direction that
  matters: a conversion whose removal changes the answer is outside
  this rule by construction, and changing the answer is exactly what
  the other rule is for.

- **`perfectionist::utf8_conversion_for_literal_comparison`** reaches
  the same comparison position, also with the UTF-8 conversions, and
  reports the operand that **carries the conversion of the value**
  where this rule reports the operand that carries the conversion of
  the **literal**. They compose rather than collide, and the shape
  that shows it is that rule's own suggestion:
  `root.file_name() == Some(OsStr::new("main.rs"))`. That
  `OsStr::new` cannot be dropped, because `Option` compares two of
  the same type, so this rule declines it by
  [(d)](#d-the-conversion-carries-nothing-else). A fix from one rule
  does not become the other's violation.

- **`perfectionist::costless_to_conversion`**,
  `perfectionist::owned_as_conversion` and
  `perfectionist::unconsumed_into_conversion` hold a conversion
  *method's name* to what its prefix promises, reading the
  declaration. This rule reads call sites and never looks at a
  method's name for anything but recognising it. A method can satisfy
  every one of those rules and still be a conversion this rule
  reports at one of its call sites.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
