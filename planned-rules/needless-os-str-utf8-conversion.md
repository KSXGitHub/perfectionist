# `needless_os_str_utf8_conversion`

**Source:** project-maintainer report. Not drawn from the
`parallel-disk-usage` / `pacquet` style guides the rest of this
catalogue extends; it captures a recurring footgun in code that
shells out to subprocesses or stores paths.

## Statement

An OS string (`OsStr`, `OsString`, `Path`, `PathBuf`) routed into a
sink that already accepts the OS-string form should be passed
through **without** a UTF-8 conversion. The two conversions this
rule fires on both throw away fidelity for nothing:

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
UTF-8 conversion of **(b)** a value of OS-string type, sitting in
**(c)** an argument position that accepts the OS-string form
directly.

### (a) The conversion

Recognised lossy / panicking conversions, all detected structurally
on the HIR (no string parsing required):

- `recv.to_string_lossy()`, plus a trailing `.into_owned()` /
  `.to_string()`.
- `recv.display()` consumed by `.to_string()` or as the sole
  non-literal piece of a `format!` / `write!` template
  (`format!("{}", recv.display())`).
- `recv.to_str()` followed by `.unwrap()` / `.expect(...)`.
- `OsString::into_string()` followed by `.unwrap()` / `.expect(...)`
  (a `PathBuf` reaches this via `.into_os_string().into_string()`).
  Its infallible cousin `PathBuf::into_os_string()` is *not* a
  trigger — it yields an `OsString`, losing nothing.

A faithful, fully-handled conversion is **not** flagged: a
`match recv.to_str() { Some(s) => …, None => … }` that copes with
the `None` arm keeps the fidelity decision explicit and is out of
scope. Only the lossy and the unwrap-on-`None` forms qualify.

### (b) The source type

The conversion's receiver must resolve (via `typeck_results`) to
`OsStr`, `OsString`, `Path`, or `PathBuf` (or a reference to one).
This guard is what makes the lossless pass-through *possible* — the
value already is an OS string, so the sink can take it as-is. It
also keeps the lint off `str`/`String` receivers that happen to
share a method name.

### (c) The sink

The argument's **formal parameter** must accept the OS string
losslessly. Detected by the parameter's trait bound at the call
site rather than a hard-coded function allowlist, so third-party and
in-house wrappers are covered for free:

- **`AsRef<OsStr>`** — `Command::{arg, args, env, envs}`,
  `command_extra::CommandExtra::{with_arg, with_args, with_env, …}`,
  `OsString::push`, and any user function with the same bound.
- **`AsRef<Path>`** — `Path::join`, `PathBuf::push`, `File::open`,
  `fs::{read, read_to_string, metadata, canonicalize, …}`.

`&Path`, `&OsStr`, `OsString`, `PathBuf`, and `Cow<OsStr>` all
satisfy both bounds, so the conversion is provably droppable.

A third category is **opt-in** behind `include_byte_sinks` (see
[Configuration](#configuration)):

- **`AsRef<[u8]>`** — `fs::write`, `io::Write::write_all`, when the
  byte argument is a converted OS string (writing a path *as file
  content*). The lossless replacement is
  `os_str.as_os_str().as_encoded_bytes()`. This is gated because the
  byte encoding of an `OsStr` is platform-specific (raw bytes on
  Unix, WTF-8 on Windows), so whether `as_encoded_bytes()` is the
  *intended* on-disk form is a judgement the rule cannot make for
  the consumer; opting in asserts "these files hold OS-string path
  data, not UTF-8 text."

### Exemptions

- Faithful, fully-handled conversions (the `match` / `?` / `ok_or`
  forms) — see (a).
- A receiver whose type is not an OS-string type — the lossless
  path does not exist, so there is nothing to suggest.
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

**Avoid** — `format!` + `unwrap` building a flag:

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

**Not flagged** — the `None` arm is handled, so the conversion is a
deliberate fidelity decision:

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
- **Byte sink (opt-in).** Suggest
  `recv.as_os_str().as_encoded_bytes()`, `MaybeIncorrect` (the
  platform-encoding caveat above).

## Configuration

```toml
[perfectionist::needless_os_str_utf8_conversion]
# Extend or restrict the sink set beyond bound-based detection.
# Function paths are absolute, so each carries a leading `::`
# per the leading-`::` convention in IMPLEMENTATION_CONVENTIONS.md.
extra_sinks = ["::my_crate::exec::spawn_with_path"]
ignore_sinks = ["::my_crate::log::record_display_path"]

# Extend / restrict the recognised conversion methods.
extra_conversion_methods = []
ignore_conversion_methods = []

# Opt into `AsRef<[u8]>` sinks (fs::write & friends). Off by
# default because the on-disk byte encoding of an OsStr is
# platform-specific; see "What to lint > (c)".
include_byte_sinks = false
```

Bound-based detection (`AsRef<OsStr>` / `AsRef<Path>`) is the
default and needs no list; `extra_sinks` / `ignore_sinks` exist for
the cases the bound check misses (a sink that takes an already-built
`OsString` by value) or over-matches (a logging helper that
*wants* the lossy text). The extras-plus-ignore shape mirrors
`perfectionist::needless_borrowed_parameters`.

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
  conversion in (a) is a method call (possibly wrapped in a trailing
  `.unwrap()` / `.expect(...)` / `.into_owned()` / `.to_string()`),
  and each sink is a call argument. Nothing here scans source text,
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
  construct, so add a `ui/needless_os_str_utf8_conversion_proc_macro.rs`
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

## Default state

Active by default. The `AsRef<OsStr>` / `AsRef<Path>` trigger does
not false-positive in practice — deliberately passing a lossily
corrupted path to a subprocess or filesystem call is essentially
never intended. The platform-nuanced byte-sink branch is held back
behind `include_byte_sinks` rather than the whole rule, so the
default policy stays purely on the objective-defect cases.

## Interaction with clippy and sibling rules

- **No clippy counterpart.** Clippy has no lint for an OS-string →
  UTF-8 conversion feeding an `AsRef<OsStr>` / `AsRef<Path>` sink,
  so this rule takes its own anti-pattern name rather than mirroring
  one.
- **`perfectionist::needless_borrowed_parameters`** shares the
  "needless conversion" theme but is orthogonal: it removes a
  `&T → T` owning conversion in a *function signature*, whereas this
  rule removes an `OsStr → str` *fidelity-destroying* conversion at
  a *call site*. Neither subsumes the other.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
