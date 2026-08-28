# `needless_utf8_conversion`

**Source:** project-maintainer report. Not drawn from the
`parallel-disk-usage` / `pacquet` style guides the rest of this
catalogue extends; it captures a recurring footgun in code that
shells out to subprocesses, stores paths, or writes byte buffers.

## Statement

A value whose native representation is *not* UTF-8 — an OS string
(`OsStr`, `OsString`, `Path`, `PathBuf`) or a byte buffer (`[u8]`,
`Vec<u8>`, `Box<[u8]>`) — routed into a sink that already accepts
that native form should be passed through **without** a UTF-8
conversion. The OS-string case is the rule's default and the running
example below; the byte-buffer case (same anti-pattern, against byte
sinks) is covered under [(b)](#b-the-source-type) and
[(c)](#c-the-sink). The conversions this rule fires on all throw away
fidelity for nothing:

- **`to_string_lossy()`** (and `Path::display().to_string()`,
  `format!("{}", path.display())`) silently replaces every
  non-UTF-8 byte with U+FFFD. The path that comes out the other end
  names a *different* file — or no file at all.
- **`to_str().unwrap()` / `.expect(...)`** (and
  `OsString::into_string().unwrap()`) panics on a perfectly valid
  filesystem path. On Unix a path is arbitrary bytes; on Windows it
  is arbitrary UTF-16; neither is guaranteed to be UTF-8.

Both are needless because the destination — `Command::arg`,
`Path::join`, `File::open`, `command_extra::CommandExtra::with_arg`,
and every other API bounded by `AsRef<OsStr>` or `AsRef<Path>` —
takes the OS string directly. `&Path`, `&OsStr`, `OsString`, and
`PathBuf` all satisfy those bounds, so the conversion can simply be
dropped (or, when a literal prefix is involved, replaced by an
`OsString` built with `OsString::push`).

The motivating shapes, corrected from the report:

**Avoid** — lossy conversion corrupts a non-UTF-8 path en route to
a subprocess argument:

```rust
use std::path::Path;
use std::process::Command;

fn build(mut command: Command, file: &Path) -> Command {
    let mut arg = "--some-flag=".to_owned();
    arg.push_str(&file.to_string_lossy()); // silent corruption
    command.arg(arg);
    command
}
```

**Avoid** — `to_str().unwrap()` panics on a valid path:

```rust
use command_extra::CommandExtra; // crate by perfectionist's author
use std::path::Path;
use std::process::Command;

fn build(command: Command, file: &Path) -> Command {
    command.with_arg(format!("--some-flag={}", file.to_str().unwrap()))
}
```

**Prefer** — build the argument as an `OsString`; nothing is lost
and nothing panics:

```rust
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

fn build(mut command: Command, file: &Path) -> Command {
    let flag = OsStr::new("--some-flag=");
    let file = file.as_os_str();
    let mut arg = OsString::with_capacity(flag.len() + file.len());
    arg.push(flag);
    arg.push(file);
    command.arg(arg);
    command
}
```

**Prefer** — the same fix through `command-extra`'s builder, which
perfectionist recognizes as a sink in its own right (see
[(c) The sink](#c-the-sink)):

```rust
use command_extra::CommandExtra;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

fn build(command: Command, file: &Path) -> Command {
    let flag = OsStr::new("--some-flag=");
    let file = file.as_os_str();
    let mut arg = OsString::with_capacity(flag.len() + file.len());
    arg.push(flag);
    arg.push(file);
    command.with_arg(arg)
}
```

Both motivating shapes above — building a `String` with `push_str`,
and interpolating the path through `format!` — are the rule's
*fuller* targets: each needs dataflow or macro-template inspection
and is a **Hard** tier in [Difficulty](#difficulty). The core,
first-shippable trigger is the simpler shape where the conversion
*is* the sink argument (e.g. `command.arg(file.to_string_lossy().into_owned())`),
shown under [Examples](#examples). All three share the same fix.

## Why is this bad?

This is a correctness issue, not a stylistic preference.

- **Silent target corruption (lossy form).** `to_string_lossy()`
  maps every ill-formed byte sequence to U+FFFD. When the result is
  handed to `Command::arg`, `File::open`, or `fs::write`, the OS
  receives a path that differs from the one the program holds.
  Best case the operation fails with "file not found" after wasting
  the work that led up to it; worst case it silently reads or writes
  the *wrong* file. The corruption is invisible at the call site —
  the program compiles, runs, and quietly does the wrong thing on
  exactly the inputs (non-UTF-8 paths) that the OS-string types
  exist to handle.
- **Needless panic (unwrap form).** `to_str().unwrap()` turns a
  representable path into a process abort. A directory walk that
  hits one non-UTF-8 entry takes down the whole program, even though
  the path could have been forwarded untouched.

Neither failure mode is hypothetical on Unix, where filenames are
arbitrary `[u8]` and non-UTF-8 names occur in the wild (legacy
encodings, deliberately adversarial names, binary-derived paths).

## What to lint

Fire on an expression that is **(a)** a fidelity-destroying
UTF-8 conversion of **(b)** a value whose native form is not UTF-8,
sitting in **(c)** an argument position that accepts that native form
directly.

### (a) The conversion

Recognised lossy / panicking conversions, all detected structurally
on the HIR (no string parsing required):

- `recv.to_string_lossy()` (returns `Cow<str>`).
- `recv.display()` consumed by `.to_string()` or as the sole
  non-literal piece of a `format!` / `write!` template
  (`format!("{}", recv.display())`).
- `recv.to_str()` followed by `.unwrap()` / `.expect(...)`.
- `OsString::into_string()` followed by `.unwrap()` / `.expect(...)`
  (a `PathBuf` reaches this via `.into_os_string().into_string()`).
  Its infallible cousin `PathBuf::into_os_string()` is *not* a
  trigger — it yields an `OsString`, losing nothing.

For **byte-buffer** sources the conversions are the same idea in
associated-function form — the value is the *argument*, not the
receiver:

- `String::from_utf8_lossy(bytes)` (lossy; returns `Cow<str>`).
- `String::from_utf8(vec)` / `str::from_utf8(bytes)` followed by
  `.unwrap()` / `.expect(...)`.

The core conversion is often followed by a trailing coercion that
adapts its result to the sink — `.into_owned()`, `.to_string()`,
`.as_str()`, `.as_ref()`, `.as_bytes()`. These are exactly what make
a lossy `Cow<str>` fit an `AsRef<OsStr>` / `AsRef<[u8]>` parameter
(the `join` and `fs::write` examples below rely on them), so the
implementation peels any such adapter to reach the core conversion.

By **default**, a faithful, fully-handled conversion is *not*
flagged — a `match recv.to_str() { Some(s) => …, None => … }`, a
`recv.to_str().ok_or(e)?`, or a
`let Some(s) = recv.to_str() else { … }` that copes with the
non-UTF-8 case. Only the lossy and the unwrap-on-`None` forms qualify
out of the box.

The `include_handled_conversions` knob (off by default — see
[Configuration](#configuration)) extends the rule to flag the handled
forms too, because "handling" the error is frequently *not* a
decision to reject non-UTF-8: it is a reflex — a developer, or an
assistant told to "replace `.unwrap()` with proper error handling",
mechanically turning `path.to_str().unwrap()` into
`path.to_str().ok_or(e)?` — that still missed the real fix, which is
not to convert at all and pass the `OsStr` through. The knob's
default and the tension behind it are explained under
[Configuration](#configuration).

### (b) The source type

The converted value must resolve (via `typeck_results`) to a type
whose native form is not UTF-8, so the lossless pass-through is
actually *possible*:

- **OS strings** — `OsStr`, `OsString`, `Path`, `PathBuf` (or a
  reference). The default sources; they reach the `AsRef<OsStr>` /
  `AsRef<Path>` sinks in (c) directly.
- **Byte buffers** — `[u8]`, `&[u8]`, `Vec<u8>`, `Box<[u8]>`. A byte
  buffer satisfies `AsRef<[u8]>` but **not** `AsRef<OsStr>` /
  `AsRef<Path>`, so it is flaggable only for the **byte sinks** gated
  behind `include_byte_sinks` (see (c)). There the
  `String::from_utf8_lossy(&bytes)` round-trip is pure waste and the
  fix (`fs::write(p, &bytes)`) has no platform nuance at all — the
  cleanest case the rule has.

The type guard also keeps the lint off genuine `str` / `String`
values that merely share a method name.

**Out of scope: `CStr` / `CString`.** They carry the same lossy /
panicking conversions (`to_string_lossy`, `to_str().unwrap()`,
`into_string().unwrap()`), but they satisfy *none* of the rule's sink
bounds — `CString: AsRef<[u8]>` does not hold, nor `AsRef<OsStr>` /
`AsRef<Path>` — and their natural consumer is FFI: a `*const c_char`
reached through `as_ptr()`, which the AsRef-bounded sink model does
not cover. A C string fed to a byte sink would have to go through its
lossless `to_bytes()` view, and "with or without the trailing NUL?"
is a real ambiguity, so the case is left out rather than guessed at.

### (c) The sink

The argument's **formal parameter** must accept the OS string
losslessly. Detected by the parameter's trait bound at the call
site rather than a hard-coded function allowlist, so third-party and
in-house wrappers are covered for free:

- **`AsRef<OsStr>`** — `Command::{arg, args, env, envs}`,
  `OsString::push`, and any function with the same bound (including
  `command-extra`'s builder, called out below).
- **`AsRef<Path>`** — `Path::join`, `PathBuf::push`, `File::open`,
  `fs::{read, read_to_string, metadata, canonicalize, …}`.

`&Path`, `&OsStr`, `OsString`, `PathBuf`, and `Cow<OsStr>` all
satisfy both bounds, so the conversion is provably droppable.

#### `command-extra` is a recognized sink by default

`command_extra::CommandExtra` is the subprocess-builder crate by
perfectionist's own author, and the rule recognizes it as a
first-class sink — *not* merely as an incidental match of the
bound-based detection. Its builder methods, which mirror std's
`Command` setters, are recognized by default:

- `with_arg`, `with_args`, `with_env`, `without_env` — the
  `AsRef<OsStr>` family.
- `with_current_dir` — the `AsRef<Path>` family.

Two things make the explicit recognition worth stating rather than
leaving implicit:

- **Precedent.** Building in knowledge of a specific well-known
  third-party crate is established in this catalogue —
  `perfectionist::manual_json_string` (`serde_json`),
  the `derive_more` rule family, `perfectionist::thiserror_usage`,
  the `clap`-help rules, `perfectionist::escaped_multiline_string`
  (`text_block`), and `perfectionist::macro_trailing_comma`'s curated
  core/std-plus-third-party list all do it. `command-extra` is the
  natural OS-string-sink analogue.
- **Shape.** `with_*` are *consuming builder methods* — they take
  the `Command` by value and return it — so a converted argument
  sits inside a `Command`-returning method chain
  (`command.with_arg(file.to_string_lossy().into_owned())`). The
  detection treats that argument position no differently from
  `Command::arg`; the recognition just guarantees the call is on the
  default sink set so coverage does not hinge on the bound-based
  path reaching a trait method's bound.

The recognition is unconditional and costs nothing when a consumer
does not depend on `command-extra` — the `with_*` calls simply never
appear. A project may still drop specific methods through
`ignore_sinks`: that list is matched against *every* sink — built-in
(std and `command-extra`), bound-detected, and `extra_sinks` alike —
and takes precedence, so an `ignore_sinks` entry naming a
`CommandExtra` method suppresses even its built-in recognition.

A third category is **opt-in** behind `include_byte_sinks` (see
[Configuration](#configuration)):

- **Byte sinks** — `fs::write` (a true `AsRef<[u8]>` bound) and the
  concrete `&[u8]` writers like `io::Write::{write_all, write}`
  (reached via the concrete-parameter arm of the detection in (c), not
  an `AsRef<[u8]>` bound). Two source families reach these:
  - **Byte buffers** (`&[u8]` / `Vec<u8>` / `Box<[u8]>`): the lossy
    `fs::write(p, String::from_utf8_lossy(&bytes).into_owned())`
    round-trip is pure waste; the fix is `fs::write(p, &bytes)`, with
    **no** platform nuance.
  - **OS strings** written *as file content*: the lossless
    replacement writes the receiver's `as_encoded_bytes()` — reached
    through `.as_os_str()` for a `Path` / `PathBuf` receiver (as in
    the `fs::write` example below), or directly on an `&OsStr`.

  The category is gated because of the OS-string source: the byte
  encoding of an `OsStr` is platform-specific (raw bytes on Unix,
  WTF-8 on Windows), so whether `as_encoded_bytes()` is the *intended*
  on-disk form is a judgement the rule cannot make; opting in asserts
  "these files hold path / raw bytes, not UTF-8 text." The
  byte-buffer source carries no such ambiguity but rides the same
  toggle, since it too only concerns byte sinks.

### Exemptions

- A converted value whose type is neither an OS string nor (for byte
  sinks) a byte buffer — it has no non-UTF-8 native form to pass
  through, so there is nothing to suggest. (This is the (b) source-type
  guard; it also keeps the lint off genuine `str` / `String` values.)
- Faithful, fully-handled conversions (`match` / `?` / `ok_or` /
  let-else) — exempt **only by default**; flagged when
  `include_handled_conversions` is enabled (see (a) and
  [Configuration](#configuration)).
- Proc-macro-synthesised nodes (see
  [Implementation notes](#implementation-notes)).

## Examples

**Avoid** — bare lossy conversion as the argument:

```rust
command.arg(file.to_string_lossy().into_owned());
```

**Prefer** — `&Path` is already `AsRef<OsStr>`:

```rust
command.arg(file);
```

**Avoid** — `format!` + `unwrap` building a flag (the macro-template
**Hard** tier — the conversion is inside the `format!`, not the sink
argument; see [Difficulty](#difficulty)):

```rust
command.arg(format!("--input={}", file.to_str().unwrap()));
```

**Prefer** — assemble the flag as an `OsString`:

```rust
let mut arg = OsString::from("--input=");
arg.push(file);
command.arg(arg);
```

**Avoid** — lossy round-trip into a path-joining sink:

```rust
base.join(name.to_string_lossy().as_ref())
```

**Prefer** — `join` already takes `AsRef<Path>`:

```rust
base.join(name)
```

**Avoid** — writing a path as file content (only with
`include_byte_sinks = true`):

```rust
fs::write(out, target.to_string_lossy().as_bytes())?;
```

**Prefer** — write the OS string's bytes directly:

```rust
fs::write(out, target.as_os_str().as_encoded_bytes())?;
```

**Avoid** — lossy round-trip writing a byte buffer (only with
`include_byte_sinks = true`):

```rust
fs::write(out, String::from_utf8_lossy(&bytes).into_owned())?;
```

**Prefer** — the bytes are already what `fs::write` wants:

```rust
fs::write(out, &bytes)?;
```

**Not flagged by default** — the `None` arm is handled. With
`include_handled_conversions = true` this *is* flagged, because the
handling could have been avoided entirely by passing the `OsStr`
(`command.arg(file)`); the rule then suggests dropping both the
conversion and its error path:

```rust
let Some(text) = file.to_str() else {
    return Err(NonUtf8Path(file.to_owned()));
};
command.arg(text);
```

## Suggested fix

- **Bare conversion as the argument.** Drop the conversion. Prefer
  `recv.as_os_str()` (for `Path`/`PathBuf`) or the receiver itself
  (for `OsStr`/`OsString`) so the edit never turns a borrow into a
  move. `Applicability::MachineApplicable` when the receiver is
  already a reference; `MaybeIncorrect` when dropping the conversion
  would move an owned value used later.
- **`format!` / concatenation with a literal prefix.** Suggest the
  `OsString` + `push` rewrite, emitted as help text rather than an
  auto-applied edit: it restructures one expression into several
  statements, which `MachineApplicable` cannot express cleanly.
- **Byte sink (opt-in).** Suggest writing the receiver's
  `as_encoded_bytes()` (reached via `.as_os_str()` for a `Path` /
  `PathBuf`, as in the `fs::write` example), `MaybeIncorrect` (the
  platform-encoding caveat above).

## Configuration

```toml
["perfectionist::needless_utf8_conversion"]
# Extend or restrict the sink set beyond bound-based detection.
# Function paths are absolute, so each carries a leading `::`
# per the leading-`::` convention in IMPLEMENTATION_CONVENTIONS.md.
extra_sinks = ["::my_crate::exec::spawn_with_path"]
ignore_sinks = ["::my_crate::log::record_display_path"]

# Extend / restrict the recognised *core* conversions (see prose
# below for how an entry maps onto the multi-step trigger shapes).
extra_conversion_methods = []
ignore_conversion_methods = []

# Opt into byte sinks (`fs::write`, `Write::write_all`, …). Off by
# default because the on-disk byte encoding of an OsStr is
# platform-specific; see "What to lint > (c)".
include_byte_sinks = false

# Also flag conversions whose non-UTF-8 case is *handled* (`?`,
# `ok_or`, `match`, let-else) but still feeds an OsStr/Path sink —
# the handling was avoidable by passing the OsStr. Off by default;
# see the rationale below.
include_handled_conversions = false
```

Bound-based detection (`AsRef<OsStr>` / `AsRef<Path>`) is the
default and needs no list; `extra_sinks` / `ignore_sinks` exist for
the cases the bound check misses (a sink that takes an already-built
`OsString` by value) or over-matches (a logging helper that
*wants* the lossy text). The extras-plus-ignore shape mirrors
`perfectionist::needless_borrowed_parameters`.

An `extra_conversion_methods` / `ignore_conversion_methods` entry
names the **core** conversion only — the fidelity-losing step listed
in (a), e.g. the receiver method `to_string_lossy` / `to_str`, or an
associated function by path (`::std::string::String::from_utf8`) for
the byte forms. It does **not** name the surrounding `.unwrap()` /
`.expect()` or the trailing coercions: those are matched (and peeled)
by the rule's fixed machinery around whatever core conversion is
recognised, so adding `"my_lossy"` makes `x.my_lossy().unwrap()` and
`x.my_lossy().as_ref()` triggers without further configuration, and
`ignore_conversion_methods = ["to_str"]` drops every `to_str`-rooted
shape while leaving `to_string_lossy` active. Method-name entries are
matched on the final segment (relative, no leading `::`); a
multi-segment associated-function path follows the leading-`::`
convention like the `*_sinks` lists. This keeps the conversion knobs
symmetric with the sink knobs rather than leaving "conversion method"
as an undefined flat string.

### Why `include_handled_conversions` is off by default

The knob sits on a genuine tension. A conversion whose non-UTF-8
case is *handled* — `path.to_str().ok_or(e)?`, a `match`, a let-else
that returns an error — and whose `&str` then feeds an OsStr/Path
sink achieved nothing the `OsStr` would not have achieved for free:
the error path is dead weight, the fix is `command.arg(path)`. That
is the case for flagging it. But the opposite reading is also
legitimate: a developer who deliberately propagated the error may
have *meant* to reject non-UTF-8 input at that boundary, in which
case the handling is intentional and the lint would be noise.

Because the two readings are indistinguishable from the syntax
alone, this is a configuration choice rather than a fixed default —
and the default follows **detection difficulty**:

- Robust, low-false-positive detection of the handled case is
  **hard**. Unlike the core trigger (where the conversion *is* the
  sink argument, a local expression match), the handled form binds
  a `&str` and feeds it to the sink later — so the rule must trace
  the binding to the sink (the same local-dataflow problem as the
  deferred "build-a-`String`-then-pass" case below) *and* prove the
  `&str` is consumed **only** by OsStr/Path sinks. If the `&str` is
  also used as text (printed, parsed, compared, stored), the
  conversion is genuinely needed and flagging it is a false
  positive. Establishing "only fed to a sink" needs whole-binding
  use-analysis.
- Per the project's policy for difficult, false-positive-prone
  triggers, a check this hard to get right ships **off by default**.
  (Had the handled case been a cheap local match with no
  false-positive risk, the opposite policy would apply and it would
  be on by default.)

An implementation may first cover the easy subset where the handling
is **inline in the argument** (`command.arg(path.to_str().ok_or(e)?)`),
which needs no dataflow and carries no other-use risk, while leaving
the out-of-line binding case for later — but the knob stays off by
default until the general case is handled soundly.

## Implementation notes

These notes record what is certain about the rule's *shape*. The
exact `rustc` / `clippy_utils` API calls are deliberately left to
the implementer to choose and verify — they have not been written or
compiled against here, so pinning specific function names would risk
sending the implementer down an unverified path.

- **`LateLintPass`, not `EarlyLintPass`.** Both halves of the
  trigger need type information that only exists after type-checking:
  confirming the conversion's receiver is an OS-string type (b), and
  confirming the sink argument accepts the OS-string form (c). An
  early pass has no types and cannot decide either.
- **Triggers are HIR expression shapes, not source text.** Every
  conversion in (a) is a method call (possibly wrapped in `.unwrap()`
  / `.expect(...)` and a trailing coercion such as `.into_owned()` /
  `.to_string()` / `.as_ref()` / `.as_bytes()`), and each sink is a
  call argument. Nothing here scans source text,
  so — unlike the markdown / serde-literal rules — the
  parser-combinator convention in
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  does not apply.
- **Detect the sink by what its parameter accepts, not by a fixed
  name list.** Recognising any argument position whose parameter
  takes the OS string losslessly (an `AsRef<OsStr>` / `AsRef<Path>`
  bound, or a concrete `&OsStr` / `OsString` / `&Path` / `PathBuf` /
  `Cow<OsStr>` parameter) is what lets std, `command-extra`, and
  in-house wrappers all be covered without an enumerated allowlist.
  `extra_sinks` / `ignore_sinks` then exist only for what this
  bound-based detection misses or over-matches. (Whether the bound
  is read off the callee signature, or approximated some other way,
  is an implementation choice to validate against real code.)
- **Proc-macro suppression.** The natural diagnostic span — the
  conversion sub-expression — is narrower than the enclosing call,
  which is exactly the case the "Suppressing proc-macro-synthesised
  violations" section of
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  flags as *not* covered by the built-in
  `report_in_external_macro: false` filter. Apply the late-pass
  `crate::common::hir_in_external_macro` guard it prescribes. A
  trigger that is realistically derive-generated is hard to
  construct, so add a `ui/needless_utf8_conversion_proc_macro.rs`
  fixture only if a non-vacuous, mutation-checked one can be built
  (delete the guard, confirm the fixture turns red); otherwise record
  at the span-selection site why it is omitted.

### Difficulty

**Medium** for the core: the direct-argument case
(`command.arg(file.to_string_lossy().into_owned())`, `base.join(p.to_str().unwrap())`)
is a local HIR-shape match plus two type queries. A conservative
first pass can ship just this — the conversion *is* the argument
expression, no dataflow.

**Hard** follow-ups, deferrable:

- The `format!` / `write!` template case (code block 2): confirm the
  template is literal apart from one flagged conversion, by
  inspecting the macro's expanded arguments.
- The build-a-`String`-then-pass case (code block 1): the conversion
  feeds a local `String` via `push_str` / `+`, and that local is
  later handed to the sink. Needs local dataflow from the
  `let mut s = String::…` binding to the sink call.
- The `include_handled_conversions` case (off by default): same
  binding-to-sink dataflow as above, *plus* proving the bound `&str`
  is consumed only by OsStr/Path sinks (else the conversion is
  genuinely needed). The inline-handling subset
  (`command.arg(path.to_str().ok_or(e)?)`) is the easy slice of this
  and can land first. See
  ["Why `include_handled_conversions` is off by default"](#why-include_handled_conversions-is-off-by-default).

## Default state

Active by default. The `AsRef<OsStr>` / `AsRef<Path>` trigger does
not false-positive in practice — deliberately passing a lossily
corrupted path to a subprocess or filesystem call is essentially
never intended. The platform-nuanced byte-sink branch is held back
behind `include_byte_sinks` rather than the whole rule, so the
default policy stays purely on the objective-defect cases.

## Interaction with clippy and sibling rules

- **No clippy counterpart.** Clippy has no lint for a non-UTF-8
  value (OS string or byte buffer) being lossily converted to UTF-8
  on its way into a sink that accepts the native form, so this rule
  takes its own anti-pattern name rather than mirroring one.
- **`perfectionist::needless_borrowed_parameters`** shares the
  "needless conversion" theme but is orthogonal: it removes a
  `&T → T` owning conversion in a *function signature*, whereas this
  rule removes an `OsStr → str` *fidelity-destroying* conversion at
  a *call site*. Neither subsumes the other.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
