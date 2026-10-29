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

## Status

The core is implemented in
[`src/rules/utf8_conversion_for_literal_comparison.rs`](../src/rules/utf8_conversion_for_literal_comparison.rs),
with the conversion recognition in
[`src/utf8_conversion.rs`](../src/utf8_conversion.rs) for the sibling
rule to reuse. This file stays as the spec for what is left.

Implemented:

- Every conversion in [(a)](#a-the-conversion) but
  `Path::display()`, each optionally under `unwrap` or `expect`.
- Every source in [(b)](#b-the-source-type).
- Both consumers in [(c)](#c-the-consumer) that collapse to an `==`
  or a `match`, the latter covering `matches!`, with the arm
  conditions of [(d)](#d-the-outcomes-the-conversion-distinguishes)
  and every exemption.
- The conversion reached through an `Option::map` or
  `Option::and_then` whose closure converts its own parameter. This
  goes beyond what [Difficulty](#difficulty) calls the core, which
  named only the conversion sitting in the operand or the scrutinee.
  It was added because the adapter is where the shape actually
  occurs: both sites the rule reports in this repository are written
  that way, and the closure is local, so none of the dataflow the
  deferred tier needs is involved.

Pending:

- **An applied fix.** The rule names the replacement in help text
  and applies nothing, so the applicability the
  [Suggested fix](#suggested-fix) section describes is not yet
  expressed. A pattern consumer's rewrite changes the scrutinee and
  every arm at once, which one span replacement cannot carry.
- **`if let` and let-else consumers.** Each collapses to one boolean
  like an `==` does, so [(d)](#d-the-outcomes-the-conversion-distinguishes)
  stays satisfied; only the HIR shapes are unread.
- **A `Result` adapter.** `Option::map` and `Option::and_then` are
  read where `Result::map` and `Result::and_then` are not, since an
  `Err` carries a payload the rewrite would have to preserve.
- Everything [Difficulty](#difficulty) lists as a follow-up:
  `starts_with` / `ends_with`, `Path::display()` consumed by
  `to_string()`, the conversion bound to a local first, and a
  conversion inside a `format!` template.

## Statement

Some values have a native representation that is not UTF-8: an OS
string (`OsStr`, `OsString`, `Path`, `PathBuf`), a byte buffer
(`[u8]`, `Vec<u8>`, `Box<[u8]>`), or a C string (`CStr`, `CString`).
Where such a value is converted to UTF-8 and **every** use of the
result is a comparison against a string literal, the comparison
belongs on the native form instead. Which spelling of the native form
depends on the consumer, and
[Suggested fix](#suggested-fix) settles that.

The conversion answers "is this valid UTF-8?" on the way to answering
"is this one of these few literals?". Nothing downstream consumes the
first answer, so the question is asked and discarded.

The motivating shape, from the commit that review produced
([`015ee9a`](https://github.com/KSXGitHub/perfectionist/commit/015ee9a8e4a8c38287683c2bc3681149d5460b9d)):

**Avoid.** The `Option` the conversion returns is threaded through
arms that are deciding between three words:

```rust
match value.to_str() {
    Some("true") => Ok(true),
    Some("false") | Some("") => Ok(false),
    _ => Err(BlessValueError(value)),
}
```

**Prefer.** The arms are the words:

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
  second question that no arm asked, which is whether the value was
  valid UTF-8. Spelling the arms as the words removes a wrapper from
  each one.
- **The validity test has no consumer.** A conversion whose `None` /
  `Err` case is folded into the same wildcard as "some other string"
  has established something the code then declines to use. A reader
  who meets `to_str` reasonably expects the encoding to matter
  somewhere, and here it does not.

Note which property of the conversion this objection does *not* rest
on. The conversion need not lose anything: the motivating shape's
`to_str()` is faithful, reports the non-UTF-8 case accurately, and is
still the thing to remove, because the comparison had no use for the
report. Fidelity is the axis
`perfectionist::needless_utf8_conversion` sorts on, and it places
this shape outside itself by default;
[Interaction with sibling rules](#interaction-with-sibling-rules)
works that through, and relates
`perfectionist::some_bool_comparison`, which makes this same
objection one type over.

### Speed is not the argument

Byte equality is cheaper for most of the sources, and the rule still
does not rest on it. `<[u8]>::eq` compares lengths before any bytes,
so a comparison against a literal of length *k* rejects a
differently-sized input without reading it, where the UTF-8
validation scans all *n* bytes first whatever *k* is. That is a real
difference in shape, and it is not a reason to change this code: the
values are a handful of bytes, read once per run, and the project
guide is explicit that measuring is how a performance question gets
settled and that sites like this are not that. Anyone tempted to
re-motivate the rule on throughput should produce a measurement
first.

A C string is the case that shows why, because there the saving is
the least secure. A NUL-terminated string's length is the index of
its terminator, so reaching its bytes is a scan in C. It is not one
through a Rust `&CStr`, which is a fat pointer carrying the length
the way `&str` does. On a 64-bit host `size_of::<&CStr>()` is 16
against 8 for a thin pointer. So the scan happened once, when the
reference was built, and `to_bytes` narrows the slice by one byte.
`CStr::to_bytes` is nevertheless documented as a constant-time cast
*today*, with the note that its definition is planned to compute the
length on each call instead; `CString::as_bytes`, slicing an owned
buffer whose length is stored, carries no such note.

Under the planned definition the rewrite would trade a UTF-8
validation for a `strlen`, which is a cheaper scan per byte but a
scan of the same length, so what is left is a constant factor rather
than the asymptotic difference the slice-backed sources give. How
large a factor has not been measured here and the rule does not need
it measured. The reason to make the rewrite is the same either way,
that being a question asked and discarded. The rule covers C
strings on that reason alone, which is the point of keeping cost out
of the rationale.

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

| Comparison                          | Byte form                        | Identical?                   |
|-------------------------------------|----------------------------------|------------------------------|
| `v.to_str() == Some(LIT)`           | `v.as_encoded_bytes() == b"LIT"` | always                       |
| `str::from_utf8(v) == Ok(LIT)`      | `v == b"LIT"`                    | always                       |
| `c.to_str() == Ok(LIT)`             | `c.to_bytes() == b"LIT"`         | always                       |
| `v.to_string_lossy() == LIT`        | `v.as_encoded_bytes() == b"LIT"` | unless `LIT` contains U+FFFD |
| `String::from_utf8_lossy(v) == LIT` | `v == b"LIT"`                    | unless `LIT` contains U+FFFD |
| `v.to_str().unwrap() == LIT`        | `v.as_encoded_bytes() == b"LIT"` | drops a panic                |

The lossy and `unwrap` rows are fidelity-destroying conversions, and
they are in scope here for the relevance reason rather than that
one: feeding a corrupted or panicking conversion into a literal
comparison corrupts nothing, since the answer is the one the faithful
conversion would have given. The `unwrap` row is the single place
where the rewrite also retires a fidelity defect, which is
`perfectionist::needless_utf8_conversion`'s reasoning arriving at a
consumer that rule does not cover.

The lossy rows are the ones with a guard worth stating precisely, and
the mechanism behind the guard is worth stating first, because the
obvious worry about a lossy conversion is the wrong one. A lossy
conversion **replaces**, and never deletes. Each ill-formed sequence
becomes U+FFFD, three bytes, so the output cannot come out shorter
than the input and an input that differs from the literal's bytes
cannot quietly shrink to match it.

What follows is the guard. A lossy output is the concatenation of the
input's valid runs, copied verbatim, with one or more U+FFFD per
ill-formed sequence. If that output equals a literal holding no
U+FFFD, then no U+FFFD was emitted, so no ill-formed sequence was
present, so the input was valid UTF-8 copied verbatim and is the
literal's bytes.

A divergence has two directions, and they are not symmetric, so each
is worth naming rather than leaving to be derived. Write the original
comparison A and the rewritten one B:

| Direction       | Possible?                              |
|-----------------|----------------------------------------|
| A true, B false | only where the literal contains U+FFFD |
| A false, B true | never, whatever the literal            |

The first direction is what the guard is for. A literal that
*contains* U+FFFD can be matched by an input that does not contain
those bytes at all, a lone `0xFF` rendering as `"\u{FFFD}"` and
comparing equal while its encoded bytes do not. A literal holding no
U+FFFD has no such pre-image, by the reasoning above.

The second direction needs no guard because it cannot happen. B true
says the value's bytes are the literal's bytes, and a string literal
is valid UTF-8, so the value is valid UTF-8 and every conversion in
[(a) The conversion](#a-the-conversion) hands back exactly that text:
the lossy ones find nothing to replace, and the faithful ones
succeed. A is then true as well. The one case where the original does
not answer true or false is the `unwrap` row on input that is not
valid UTF-8, where it panics rather than compares, which
[Suggested fix](#suggested-fix) prices into that row's applicability.

So the literal's own content is the whole condition, and the
condition is a scan of it, which makes it a guard the rule applies
rather than a knob the consumer sets.

The replacement property was checked rather than argued, and
exhaustively, because the whole guard rests on it. Over all 16843008
byte strings of length 1 to 3: every input that is not valid UTF-8
left a U+FFFD in its output, and no input produced an output shorter
than itself. Over all byte strings of length 3, the only one whose
lossy form is `"yes"` is `b"yes"`, and no perturbation of `b"yes"` is
another, taking every byte inserted at every position and every byte
substituted at every position. An ill-formed byte beside the word
behaves as the mechanism says: `b"yes\xFF"` renders as
`"yes\u{FFFD}"`, `b"yes\xC3"` likewise renders as `"yes\u{FFFD}"`,
and neither equals `"yes"`.

Each direction was checked separately, over the same length 1 to 3
inputs. For A true and B false: zero cases against the literals
`"y"`, `"ye"`, `"yes"` and `"é"`, and 17728 against `"\u{FFFD}"` with
1344 against `"a\u{FFFD}"`, which is the guard earning its place
rather than failing. For A false and B true: zero cases over all
2668544 valid-UTF-8 inputs, counting the lossy conversions of a byte
buffer and of an OS string, the faithful `to_str`, and `to_str`
followed by `unwrap`, each against the literal its own bytes spell.

Each row was then checked over an input set spanning the words
themselves, ASCII that differs in case, valid non-ASCII UTF-8 (`é`),
a lone `0xFF`, a truncated U+FFFD prefix, an unpaired surrogate, and
the empty string, against literals including `é` and U+FFFD. The
faithful rewrites disagreed on nothing, and the lossy rewrites
disagreed only on the U+FFFD-literal cases the guard excludes. The
cross-platform half of the claim rests on the documented guarantee
rather than on that run, which was Linux only.

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
view to compare against, which a pattern consumer has to use and an
equality consumer may skip, per
[Suggested fix](#suggested-fix):

| Source                 | Byte view                             |
|------------------------|---------------------------------------|
| `OsStr`, `OsString`    | `as_encoded_bytes()`                  |
| `Path`, `PathBuf`      | `as_os_str().as_encoded_bytes()`      |
| `[u8]`, `&[u8]`        | the value itself                      |
| `Vec<u8>`, `Box<[u8]>` | the value, dereferenced for a pattern |
| `CStr`                 | `to_bytes()`                          |
| `CString`              | `as_bytes()`                          |

`as_encoded_bytes` is the view to suggest rather than the
`OsStrExt::as_bytes` that review's opening question named. On Unix the
choice is invisible: the two hand back the same slice, same pointer
and same length, for every byte sequence including ones that are not
valid UTF-8. Checked on this host over all 256 byte values.

What separates them is the contract rather than the bytes, and it
shows up on the way back. `OsStrExt::from_bytes` is safe, because on
Unix any byte sequence is a valid OS string, while
`OsStr::from_encoded_bytes_unchecked` is `unsafe`, because the
encoding it accepts is unspecified. This rule only ever reads the
bytes forward and compares them, never reconstructing an `OsStr` from
them, so the unsafe inverse and the opacity caveat that motivates it
never arise.

Portability is what settles it. `OsStrExt::as_bytes` is Unix-only, and
the Windows `OsStrExt` offers no byte view to stand in for it: its one
method is `encode_wide`, which yields `u16` code units and so cannot
meet a byte-string literal at all. `as_encoded_bytes` is an inherent
method on `OsStr`, present on every target. There it reads the storage
`OsString` documents as "a less-strict variant of UTF-8", the encoding
usually called WTF-8, which keeps every valid-UTF-8 sequence spelled
as itself and gives an unpaired surrogate a three-byte form that UTF-8
rejects. A literal therefore compares the same way on both platforms,
and a surrogate fails `to_str` and the byte comparison alike.

C strings are **in** scope here, which is a deliberate divergence
from `perfectionist::needless_utf8_conversion`. That rule excludes
them because they satisfy none of its sink bounds and because
"with or without the trailing NUL?" has no safe default answer when
the bytes are being handed onward. Against a literal the question
settles itself: a string literal cannot contain a NUL terminator, so
the view that excludes the terminator is the only one that can
compare equal.

Which view that is has to be stated rather than left to whoever
writes the suggestion, because the alternative is adjacent and fails
quietly. `CStr::to_bytes` and `CString::as_bytes` exclude the
terminator, so the literal stays plain: for a `CStr` holding
`"true\0"`, `to_bytes() == b"true"` holds. Their
`to_bytes_with_nul` / `as_bytes_with_nul` counterparts retain it, and
`to_bytes_with_nul() == b"true"` is **false** for that same C string.
That comparison compiles, never matches, and reports nothing. A
suggestion reaching for a `*_with_nul` view would have to emit
`b"true\0"` to work at all, which states a terminator the original
comparison never mentioned. So the `*_with_nul` family is not a view
this rule may suggest, and an implementation that recognises one as a
*source* of bytes must not pair it with a plain literal.

Checked on this host: `to_bytes()` agreed with `to_str() == Ok(LIT)`
on every C string tried, against literals including `é` and the
empty one. The inputs spanned the words themselves, differing case, a
`0xFF` byte, valid non-ASCII UTF-8, and the empty C string.

### (c) The consumer

Every use of the conversion's result must be one of these:

- An `==` or `!=` whose other operand is a string literal, or a
  `Some` / `Ok` of one. Either operand may be the literal.
- A `match`, `matches!`, `if let` or let-else whose patterns are
  string literals, `Some` / `Ok` of string literals, and wildcards,
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
same outcome, by way of a single wildcard arm covering both. The
`_ => Err(BlessValueError(value))` of the motivating shape is such an
arm. A `match` that gives the non-UTF-8 case an arm of its own is
using the conversion for what it is for, and is not a violation.

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

**Avoid.** An `Option` wrapping a question about one word:

```rust
if value.to_str() == Some("true") { .. }
```

**Prefer:**

```rust
if value.as_encoded_bytes() == b"true" { .. }
```

**Avoid.** A `Path` receiver, reached through its OS string:

```rust
if path.to_str() == Some("-") { .. }
```

**Prefer:**

```rust
if path.as_os_str().as_encoded_bytes() == b"-" { .. }
```

**Avoid.** A byte buffer validated to be compared:

```rust
matches!(str::from_utf8(bytes), Ok("yes" | "no"))
```

**Prefer.** The buffer is already what the comparison wants:

```rust
matches!(bytes, b"yes" | b"no")
```

**Avoid.** A C string, whose byte view drops the terminator the
literal never mentions:

```rust
if c_str.to_str() == Ok("yes") { .. }
```

**Prefer:**

```rust
if c_str.to_bytes() == b"yes" { .. }
```

**Avoid.** A lossy conversion, with no U+FFFD in the literal:

```rust
if String::from_utf8_lossy(bytes) == "yes" { .. }
```

**Prefer:**

```rust
if bytes == b"yes" { .. }
```

**Avoid.** A panicking conversion, whose rewrite also removes the
panic:

```rust
if os_string.into_string().unwrap() == "yes" { .. }
```

**Prefer:**

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

The consumer decides which spelling the fix takes, and for an
OS-string source the choice is real rather than cosmetic.

### A pattern consumer forces the byte view

A `match`, `matches!`, `if let` or let-else needs its literal in
pattern position, and there is no `OsStr` literal to put there:
`OsStr::new("x")` is a function call, which a pattern cannot be. So
the scrutinee becomes the source's byte view from
[(b)](#b-the-source-type) and every arm becomes a byte-string
literal. The shape in [Statement](#statement) is this case, which is
why its fix reads `as_encoded_bytes`.

### An `==` consumer should compare as `OsStr`

`OsStr` implements `PartialEq`, so an equality against a literal can
keep the literal exactly as written:

**Avoid:**

```rust
if value.to_str() == Some("true") { .. }
```

**Prefer:**

```rust
if value == OsStr::new("true") { .. }
```

Nothing is respelled, so none of the literal constraints below
arise, and the reader still sees one comparison against the word
rather than a comparison of byte views. Prefer this wherever the
source is an OS string and the consumer is `==` or `!=`. It costs
nothing: `OsStr::new` returns a borrow whose lifetime is its
argument's, so it cannot allocate and hand back a reference to the
allocation.

A `Path` or `PathBuf` source reaches this spelling through
`as_os_str()`, and the detour is **not** optional.

> [!WARNING]
> `Path::new` looks like the counterpart and is not one. `Path`
> compares by normalised components rather than by bytes, so
> `Path::new("a/b") == Path::new("a//b")` holds, as do the `.` and
> trailing-separator cases `Path::components` documents. That makes
> `p == Path::new(LIT)` a *weaker* predicate than
> `p.to_str() == Some(LIT)`, so a rule suggesting it would rewrite a
> consumer's comparison into one that answers differently.

Checked over paths including `a//b`, `a/./b` and `main.rs/` against
the literals they normalise onto: `p.as_os_str() == OsStr::new(LIT)`
disagreed with `p.to_str() == Some(LIT)` on nothing, for `&Path`,
`PathBuf`, `&OsStr` and `OsString` receivers alike, while
`p == Path::new(LIT)` disagreed on three.

A byte buffer needs no counterpart to `OsStr::new`, being already
its own native form. Its literal is a byte-string literal, and the
constraints below are what settle its spelling.

### Emitting a byte-string literal

Where the fix does emit one, two constraints apply:

- **`b"…"`, never `"…".as_bytes()`.** `clippy::string_lit_as_bytes`
  flags `"…".as_bytes()` and suggests `b"…"`, so a fix reaching for
  the `as_bytes` spelling would trade this lint for that one.
- **A non-ASCII literal needs escapes.** `b"é"` does not compile
  ("non-ASCII character in byte string literal … must be ASCII"), so
  a literal outside 7-bit ASCII is emitted as the `\xHH` escapes of
  its UTF-8 bytes, so `é` is emitted as `b"\xc3\xa9"`.

Applicability follows the identity table in
[The rewrite is an identity](#the-rewrite-is-an-identity):
`MachineApplicable` where the rewrite computes the answer the
conversion did, `MaybeIncorrect` where it removes a panic, since a
program relying on that panic to reject non-UTF-8 input is using the
conversion for something the comparison does not show.

## Configuration

None. The conversions are a closed set of std functions, and the
comparison is an operator or a pattern. One condition might otherwise
invite a knob, which is whether the lossy forms are safe to rewrite.
Reading the literal settles that per site, so taste never has to
settle it per project.

## Default state

Active by default. The conditions in [What to lint](#what-to-lint)
are each a local property of one expression. The conditions carrying
the false-positive risk, a bound arm and a distinguished non-UTF-8
outcome, are exclusions rather than heuristics. So a site that
satisfies all of them is a violation and not a guess.

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
  comparison. That is the case the
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
  literal. The `str` method's other `Pattern` arguments have no byte
  counterpart and are outside the trigger, a closure and a `char`
  predicate among them.
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
[Examples](#examples) under `clippy::all`, `clippy::pedantic`,
`clippy::nursery` and `clippy::restriction` together. Those shapes
cover the OS-string, byte-buffer, equality, `match`, `matches!`,
`if let` and `unwrap` forms. Nothing fired on the conversion. The
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
  suggestion. Both were confirmed by running them.
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

- **`perfectionist::needless_utf8_conversion`** sorts the same
  conversions on a different axis, which is what keeps the two rules
  apart more cleanly than their consumers do.

  That rule's axis is **fidelity**. It recognises the conversions
  that lose something, among them `to_string_lossy()`,
  `display().to_string()` and the `unwrap`-on-`None` forms. It then
  says in as many words that a faithful, fully-handled conversion is
  not flagged by default, naming
  `match recv.to_str() { Some(s) => …, None => … }` as a shape it
  leaves alone. Its rationale follows from that axis: a corrupted
  path reaching the OS names the wrong file, and a panic on a valid
  path is a needless abort.

  This rule's axis is **relevance**. What it objects to is a question
  asked and discarded, so a conversion qualifies however faithful it
  is. The motivating shape is a bare `to_str()` under a `match`,
  which loses nothing and reports the non-UTF-8 case accurately, so
  the fidelity axis places it outside itself. The two
  default trigger sets are therefore close to complementary rather
  than overlapping: a `to_str()` with no `unwrap` is this rule's
  core and that rule's documented exemption.

  The consumers differ too, and that is what makes the triggers
  provably disjoint rather than merely differently motivated: a call
  argument in a position whose bound accepts the native form is not a
  comparison operand. The fixes differ with them: dropping the
  conversion there, replacing it with a lossless byte view and
  respelling the literals here. Neither rule subsumes the other, and
  the two reach opposite conclusions about `CStr` for the reason
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
