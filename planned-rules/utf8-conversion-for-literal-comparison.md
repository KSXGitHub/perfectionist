# `utf8_conversion_for_literal_comparison`

**Source:** this repository's own review history.
<https://github.com/KSXGitHub/perfectionist/pull/579#discussion_r4402980739>
asked, of a `PERFECTIONIST_BLESS` parser that matched
`value.to_str()` against three words:

> `to_str` performs some UTF-8 algorithm, right?
>
> But if `as_bytes`, it can be matched against `b"true"` and
> `b"false"` instead.

The reply in
<https://github.com/KSXGitHub/perfectionist/pull/579#discussion_r4403022017>
confirmed it, named what the rewrite actually buys, and recorded what
it does *not* buy. Both halves are load-bearing here: the rationale
under [Why restrict this?](#why-restrict-this) is the one that review
settled on, and the one it rejected is written down so a later reader
does not reintroduce it.

## Statement

A value whose native representation is *not* UTF-8 — an OS string
(`OsStr`, `OsString`, `Path`, `PathBuf`), a byte buffer (`[u8]`,
`Vec<u8>`, `Box<[u8]>`), or a C string (`CStr`, `CString`) — converted
to UTF-8 where **every** use of the result is a comparison against a
string literal should be compared in its native byte form instead.

The conversion answers "is this valid UTF-8?" on the way to answering
"is this one of these few literals?". Nothing downstream consumes the
first answer, so the question is asked and discarded.

The motivating shape, from the commit that review produced
([`015ee9a`](https://github.com/KSXGitHub/perfectionist/commit/015ee9a8e4a8c38287683c2bc3681149d5460b9d)):

**Avoid** — the `Option` the conversion returns is threaded through
arms that are deciding between three words:

```rust
match value.to_str() {
    Some("true") => Ok(true),
    Some("false") | Some("") => Ok(false),
    _ => Err(BlessValueError(value)),
}
```

**Prefer** — the arms are the words:

```rust
match value.as_encoded_bytes() {
    b"true" => Ok(true),
    b"false" | b"" => Ok(false),
    _ => Err(BlessValueError(value)),
}
```

## Why restrict this?

This is a stylistic preference, not a correctness issue. For every
form the rule fires on by default the two spellings compute the same
answer on every input, which
[The rewrite is an identity](#the-rewrite-is-an-identity) establishes
case by case. The objection is to what the reader has to carry:

- **A `Some` that has nothing to do with the question.** The match is
  deciding which of three words the value is. Threading the
  conversion's `Option` through the arms makes every arm answer a
  second question — "and was it valid UTF-8?" — that no arm asked.
  Spelling the arms as the words removes a wrapper from each one.
- **The validity test has no consumer.** A conversion whose `None` /
  `Err` case is folded into the same wildcard as "some other string"
  has established something the code then declines to use. A reader
  who meets `to_str` reasonably expects the encoding to matter
  somewhere, and here it does not.

`perfectionist::some_bool_comparison` makes this objection one type
over; [Interaction with sibling rules](#interaction-with-sibling-rules)
says how the two relate.

### Speed is not the argument

Byte equality is cheaper, and the rule still does not rest on it.
`<[u8]>::eq` compares lengths before any bytes, so a comparison
against a literal of length *k* rejects a differently-sized input
without reading it, where the UTF-8 validation scans all *n* bytes
first whatever *k* is. That is a real difference in shape, and it is
not a reason to change this code: the values are a handful of bytes,
read once per run, and the project guide is explicit that measuring
is how a performance question gets settled and that sites like this
are not that. Anyone tempted to re-motivate the rule on throughput
should produce a measurement first.

### The rewrite is an identity

`OsStr::as_encoded_bytes` is documented as yielding "an unspecified,
platform-specific, self-synchronizing superset of UTF-8" and,
following from that, "also a superset of 7-bit ASCII". A string
literal is valid UTF-8 by construction, so it is encoded as its own
UTF-8 bytes on every target, and an OS string that is *not* valid
UTF-8 cannot equal it. The caveat attached to the method is about
carrying non-UTF-8 *fragments* to another machine or another Rust
version, which an equality test against a literal does not do.

That gives the per-conversion picture, in which `LIT` stands for any
string literal and `b"LIT"` for that literal respelled as a
byte-string literal:

| Comparison                       | Byte form                       | Identical? |
|----------------------------------|---------------------------------|------------|
| `v.to_str() == Some(LIT)`        | `v.as_encoded_bytes() == b"LIT"` | always |
| `str::from_utf8(v) == Ok(LIT)`   | `v == b"LIT"`                    | always |
| `v.to_string_lossy() == LIT`     | `v.as_encoded_bytes() == b"LIT"` | unless `LIT` contains U+FFFD |
| `v.to_str().unwrap() == LIT`     | `v.as_encoded_bytes() == b"LIT"` | drops a panic |

The lossy row is the one with a guard worth stating precisely.
`to_string_lossy` maps each ill-formed sequence to U+FFFD, so a
literal that *contains* U+FFFD can be matched by an input that does
not contain those bytes at all: a lone `0xFF` renders as `"\u{FFFD}"`
and compares equal, while its encoded bytes do not. A literal with no
U+FFFD in it has no such pre-image, and the rewrite is exact. The
condition is a scan of the literal, so it is a guard the rule applies
rather than a knob the consumer sets.

The `to_str` and `to_string_lossy` rows were checked on this host
over an input set spanning the words themselves, ASCII that differs
in case, valid non-ASCII UTF-8 (`é`), a lone `0xFF`, a truncated
U+FFFD prefix, an unpaired surrogate, and the empty string, against
literals including `é` and U+FFFD: the `to_str` rewrite disagreed on
nothing, and the `to_string_lossy` rewrite disagreed only on the
U+FFFD-literal cases the guard excludes. The cross-platform half of the claim rests
on the documented guarantee rather than on that run, which was Linux
only.

## What to lint

Fire on **(a)** a UTF-8 conversion of **(b)** a value whose native
form is not UTF-8, where **(c)** every use of the conversion's result
is a comparison against a string literal and **(d)** the outcomes the
conversion distinguishes survive the rewrite.

### (a) The conversion

Receiver methods, where the converted value is the receiver:

- `to_str()`, yielding `Option<&str>` on an OS string and
  `Result<&str, _>` on a C string.
- `to_string_lossy()`, yielding `Cow<str>`.
- `OsString::into_string()`, yielding `Result<String, OsString>`. A
  `PathBuf` reaches it through `into_os_string()`.
- `Path::display()` consumed by `to_string()`, which is
  `to_string_lossy` wearing a `Display` impl.

Associated functions, where the converted value is the argument:

- `str::from_utf8(v)` / `String::from_utf8(v)`, yielding a `Result`.
- `String::from_utf8_lossy(v)`, yielding `Cow<str>`.

A `.unwrap()` / `.expect(..)` on the fallible forms, and a trailing
coercion that adapts the result to the comparison (`.as_str()`,
`.into_owned()`, `.as_ref()`), sit around the core conversion rather
than replacing it; the implementation peels them to reach it.

### (b) The source type

Resolved through `typeck_results`, so a `str` that merely shares a
method name is never flagged. Each native form has a lossless byte
view to compare against:

| Source                              | Byte view |
|-------------------------------------|-----------|
| `OsStr`, `OsString`                 | `as_encoded_bytes()` |
| `Path`, `PathBuf`                   | `as_os_str().as_encoded_bytes()` |
| `[u8]`, `&[u8]`                     | the value itself |
| `Vec<u8>`, `Box<[u8]>`              | the value, dereferenced for a pattern |
| `CStr`, `CString`                   | `to_bytes()` |

`as_encoded_bytes` is the view to suggest rather than the
`OsStrExt::as_bytes` that review's opening question named: the two
agree on Unix, and `as_encoded_bytes` is the one that also exists on
Windows.

C strings are **in** scope here, which is a deliberate divergence
from `perfectionist::needless_utf8_conversion`. That rule excludes
them because they satisfy none of its sink bounds and because
"with or without the trailing NUL?" has no safe default answer when
the bytes are being handed onward. Against a literal the question
settles itself: a string literal cannot contain a NUL terminator, so
`to_bytes()` — the view that excludes it — is the only one that can
compare equal.

### (c) The consumer

Every use of the conversion's result must be one of these:

- An `==` or `!=` whose other operand is a string literal, or a
  `Some` / `Ok` of one. Either operand may be the literal.
- A `match`, `matches!`, `if let` or let-else whose patterns are
  string literals, `Some` / `Ok` of string literals, and wildcards —
  with **no** arm binding the decoded text. An arm that binds it
  (`Some(other) => f(other)`, or a guard reading the binding) is
  using the value as text, which is what the conversion is for.

Where the conversion is not itself the comparison operand but is
bound to a local first, establishing "every use" needs local
dataflow; [Difficulty](#difficulty) defers that case.

### (d) The outcomes the conversion distinguishes

A `match` on `Option<&str>` or `Result<&str, _>` can tell three
inputs apart:

1. one of the listed literals,
2. valid UTF-8 that is none of them,
3. not valid UTF-8.

A byte match separates 1 from everything else and cannot separate 2
from 3. So the rule may fire only where 2 and 3 already reach the
same outcome — a single wildcard arm covering both, as the
`_ => Err(BlessValueError(value))` of the motivating shape does. A
`match` that gives the non-UTF-8 case an arm of its own is using the
conversion for what it is for, and is not a violation.

An `==`, a `matches!`, an `if let` and a let-else each collapse to
one boolean, leaving nothing to distinguish, so this condition is
only ever live for a `match`.

### Exemptions

- A comparand that is not a literal. There is nothing to rewrite the
  comparison against.
- An arm that binds the decoded text, or any other use of the result
  that reads it as text.
- A `match` whose non-UTF-8 case has an outcome of its own, per
  [(d)](#d-the-outcomes-the-conversion-distinguishes).
- A lossy conversion compared against a literal containing U+FFFD,
  per [The rewrite is an identity](#the-rewrite-is-an-identity).
- Proc-macro-synthesised nodes, per
  [Implementation notes](#implementation-notes).

## Examples

### The shape

**Avoid** — an `Option` wrapping a question about one word:

```rust
if value.to_str() == Some("true") { .. }
```

**Prefer**:

```rust
if value.as_encoded_bytes() == b"true" { .. }
```

**Avoid** — a `Path` receiver, reached through its OS string:

```rust
if path.to_str() == Some("-") { .. }
```

**Prefer**:

```rust
if path.as_os_str().as_encoded_bytes() == b"-" { .. }
```

**Avoid** — a byte buffer validated to be compared:

```rust
matches!(str::from_utf8(bytes), Ok("yes" | "no"))
```

**Prefer** — the buffer is already what the comparison wants:

```rust
matches!(bytes, b"yes" | b"no")
```

**Avoid** — a lossy conversion, with no U+FFFD in the literal:

```rust
if String::from_utf8_lossy(bytes) == "yes" { .. }
```

**Prefer**:

```rust
if bytes == b"yes" { .. }
```

**Avoid** — a panicking conversion, whose rewrite also removes the
panic:

```rust
if os_string.into_string().unwrap() == "yes" { .. }
```

**Prefer**:

```rust
if os_string.as_encoded_bytes() == b"yes" { .. }
```

### Not flagged

**The decoded text is used as text.** The `Some(other)` arm reads the
`&str`, so the conversion earns its place:

```rust
match path.to_str() {
    Some("-") => Source::Stdin,
    Some(other) => Source::Named(other.to_owned()),
    None => return Err(NonUtf8Path(path.to_owned())),
}
```

**The non-UTF-8 case has an outcome of its own.** No arm binds the
text, but a byte match cannot keep `Invalid` apart from `Other`:

```rust
match path.to_str() {
    Some("-") => Kind::Dash,
    Some(_) => Kind::Other,
    None => Kind::Invalid,
}
```

**The comparand is not a literal.** Nothing here can be spelled as a
byte-string literal:

```rust
if path.to_str() == Some(wanted) { .. }
```

**A lossy conversion against a literal holding U+FFFD.** A lone
`0xFF` renders as U+FFFD and compares equal, where its encoded bytes
do not, so the rewrite would change the answer:

```rust
if path.to_string_lossy() == "\u{FFFD}" { .. }
```

**The receiver is already a `str`.** The source-type guard in
[(b)](#b-the-source-type) keeps the lint off a value with no
non-UTF-8 native form:

```rust
if text.to_lowercase() == "true" { .. }
```

## Suggested fix

Replace the conversion with the source's byte view from
[(b)](#b-the-source-type) and rewrite each literal in the comparison
as a byte-string literal.

Emitting the literal has constraints that a suggestion has to
respect:

- **`b"…"`, never `"…".as_bytes()`.** `clippy::string_lit_as_bytes`
  flags `"…".as_bytes()` and suggests `b"…"`, so a fix reaching for
  the `as_bytes` spelling would trade this lint for that one.
- **A non-ASCII literal needs escapes.** `b"é"` does not compile
  ("non-ASCII character in byte string literal … must be ASCII"), so
  a literal outside 7-bit ASCII is emitted as the `\xHH` escapes of
  its UTF-8 bytes — `b"\xc3\xa9"` for `é`.

Applicability follows the identity table in
[The rewrite is an identity](#the-rewrite-is-an-identity):
`MachineApplicable` where the rewrite computes the answer the
conversion did, `MaybeIncorrect` where it removes a panic, since a program relying
on that panic to reject non-UTF-8 input is using the conversion for
something the comparison does not show.

## Configuration

None. The conversions are a closed set of std functions, the
comparison is an operator or a pattern, and the one condition a
project might otherwise want to tune — whether the lossy forms are
safe to rewrite — is decided per site by reading the literal rather
than per project by taste.

## Default state

Active by default. The conditions in
[What to lint](#what-to-lint) are each a local property of one
expression, and the conditions carrying the false-positive risk — a
bound arm and a distinguished non-UTF-8 outcome — are exclusions
rather than heuristics, so a site that satisfies all of them is a
violation and not a guess.

## Implementation notes

These notes record what is certain about the rule's *shape*. The
`rustc` / `clippy_utils` calls are left to the implementer to choose
and verify, since none of them has been compiled against here.

- **`LateLintPass`, not `EarlyLintPass`.** The source-type condition
  in [(b)](#b-the-source-type) needs `typeck_results`, which an early
  pass does not have.
- **Triggers are HIR expression shapes, not source text.** Each
  conversion is a call and each consumer is a binary operation or a
  pattern, so the parser-combinator convention in
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  does not apply. The one piece of text handling is spelling the
  replacement literal, which reads the literal's own bytes rather
  than scanning source.
- **The conversion set is shared with
  `perfectionist::needless_utf8_conversion`.** Both rules recognise
  the same conversions in (a) and differ in what consumes them, so
  whichever ships first factors the recognition into a
  crate-internal module for the other to reuse rather than
  duplicating the list.
- **Proc-macro suppression.** The natural diagnostic span is the
  conversion sub-expression, which is narrower than the enclosing
  comparison — the case the
  [proc-macro suppression convention](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations)
  identifies as uncovered by `report_in_external_macro: false`.
  Apply the late-pass `crate::common::hir_in_external_macro` guard it
  prescribes, and add a
  `ui/utf8_conversion_for_literal_comparison_proc_macro.rs` fixture
  only if a non-vacuous one can be built: delete the guard and
  confirm the fixture turns red. Otherwise record at the
  span-selection site why it is omitted.

### Difficulty

**Medium** for the core, which is the case where the conversion *is*
the comparison operand or the match scrutinee: a local HIR-shape
match, one type query, and a walk over the arm patterns. The
motivating shape is in this tier.

**Medium** follow-ups, each independent of the core:

- `starts_with` / `ends_with` against a literal.
  `<[u8]>::starts_with` takes a slice, so a string-literal argument
  translates and an ASCII `char` literal translates to a one-byte
  literal, while the `str` method's other `Pattern` arguments — a
  closure, a `char` predicate — have no byte counterpart and are
  outside the trigger.
- `Path::display()` consumed by `to_string()`, which needs the
  `Display` impl recognised as the lossy conversion it wraps.

**Hard** follow-ups, deferrable:

- The conversion bound to a local before the comparison. Proving
  that *every* use of the binding is a literal comparison is the
  local-dataflow problem, and the "every use" wording in
  [(c)](#c-the-consumer) is what it has to establish.
- A conversion inside a `format!` or `write!` template compared
  afterwards. The comparison is against the rendered string rather
  than the converted value, so the trigger would have to read the
  template.

## Interaction with Clippy and rustc

**Neither has a lint for this.** Checked against
`clippy-driver 0.1.97` by running the shapes in
[Examples](#examples) — the OS-string, byte-buffer, equality,
`match`, `matches!`, `if let` and `unwrap` forms — under
`clippy::all`, `clippy::pedantic`, `clippy::nursery` and
`clippy::restriction` together. Nothing fired on the conversion. The
only diagnostics were about the probe's own unrelated style
(`clippy::ptr_arg`, `clippy::unwrap_used`, `clippy::must_use_candidate`
and others of that kind), and the suggested replacements drew no
diagnostic at all.

The lints that come closest, and why each one leaves the anti-pattern
standing:

- **`clippy::cmp_owned`** flags an owned value *created* for a
  comparison, as in `x == "foo".to_string()`. It does not fire on any
  form here: the conversions it looks for are `to_owned`-shaped, and
  a `to_str` that allocates nothing is not among them.
- **`clippy::needless_as_bytes`** fires on `as_bytes()` followed by
  `len()` or `is_empty()`, where the `str` method answers the same
  question. It is scoped to those two calls and does not reach a
  comparison, so it neither covers this rule nor objects to its
  suggestion — confirmed by running both.
- **`clippy::string_lit_as_bytes`** fires on `"lit".as_bytes()` and
  suggests `b"lit"`. It constrains how this rule spells its
  suggestion, per [Suggested fix](#suggested-fix), rather than
  overlapping with its trigger.
- **`clippy::string_from_utf8_as_bytes`** flags a `str` round-tripped
  through its own bytes. The direction is reversed: it removes a byte
  view obtained from UTF-8 text, where this rule removes UTF-8 text
  obtained from a byte view.
- **`invalid_from_utf8`**, a rustc built-in, flags `str::from_utf8`
  applied to a literal that cannot be valid UTF-8. It judges the
  argument, not what the result is compared against.

The name is therefore this rule's own rather than a Clippy name
borrowed, which is what the
[name-mirroring convention](./IMPLEMENTATION_CONVENTIONS.md#mirror-the-clippy-name-only-for-a-genuine-refinement)
asks for when no counterpart exists.

## Interaction with sibling rules

- **`perfectionist::needless_utf8_conversion`** recognises the same
  conversions and differs in what consumes them. There the consumer
  is a parameter position whose bound already accepts the native form
  and the fix drops the conversion outright; here the consumer is a
  comparison against literals and the fix replaces the conversion
  with a lossless byte view and rewrites the literals. The triggers
  are disjoint — a call argument is not a comparison operand — so
  neither rule subsumes the other, and the two reach opposite
  conclusions about `CStr`, for the reason
  [(b)](#b-the-source-type) records.
- **`perfectionist::some_bool_comparison`** is the same objection one
  type over: it flags `Option<bool>` compared against `Some` of a
  boolean literal and names the `unwrap_or` that states what `None`
  means. Both rules object to a `Some` threaded through a comparison
  that is not asking about presence. They never overlap, since a
  conversion to UTF-8 does not produce an `Option<bool>`.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
