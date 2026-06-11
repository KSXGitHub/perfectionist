# `prefer_owned_parameter`

**Source:** pacquet *When to use owned parameter? When to use
borrowed parameter?*. The pacquet guide gives both directions of
the trade-off; this lint covers the *prefer owned* direction. The
opposite direction ("prefer borrowed when ownership isn't consumed")
is already covered by `clippy::needless_pass_by_value`.

## Status

Partially implemented in `src/rules/prefer_owned_parameter.rs`. The
conservative single-use starting point described under
["Difficulty"](#difficulty) is what ships today.

**Implemented:**

- Shared-reference parameters (`&str`, `&Path`, `&OsStr`, `&CStr`,
  `&[T]`) whose owned counterpart is `String`, `PathBuf`, `OsString`,
  `CString`, `Vec<T>`.
- The parameter is referenced *exactly once*, that use is the
  conversion, and the conversion's result type really is the owned
  counterpart (so a same-named method that returns something else does
  not trigger it).
- Conversion shapes: `to_owned`, `to_string`, `to_path_buf`, `to_vec`,
  `to_os_string`, `clone`, `into` (configurable), plus the
  `Owned::from(param)` free-function form.
- The conversion must run unconditionally — it is not nested inside an
  `if` / `match` / loop arm, a closure, or the short-circuiting side of
  `&&` / `||`.
- Exemptions: trait-declaration and trait-`impl` methods, parameters
  with an explicit named lifetime, and proc-macro-synthesised nodes
  (`hir_in_external_macro` guard, regression fixture
  `ui/prefer_owned_parameter_proc_macro.rs`).
- Configuration: `extra_conversion_methods` / `ignore_conversion_methods`
  (the repository's extras-plus-ignore convention) rather than the
  single replacement list sketched under "Configuration" below.

**Still pending:**

- The multi-use / branching cases that need the control-flow dominance
  analysis (the parameter used several times, all dominated by a
  conversion).
- `&mut T` parameters.
- The `type_pairs` knob for project-specific borrowed/owned newtypes;
  only the standard-library pairs are recognised today.
- The `&String` / `&Vec<T>` / `&PathBuf` + `.clone()` direction, which
  overlaps `clippy::ptr_arg` and is not flagged.

## Statement

When a function parameter is borrowed (`&T`, `&str`, `&Path`,
`&[U]`, …) but the function body unconditionally converts it to the
owned form (`.to_owned()`, `.to_string()`, `.to_path_buf()`,
`.to_vec()`, `.clone()`, `T::from(param)`), the function should
take the owned form directly. Taking owned eliminates the copy when
the caller already has an owned value, while preserving the same
ergonomics for callers that have a borrowed value (they just call
`.to_owned()` themselves).

The pacquet example, slightly condensed.

**Avoid:** the borrowed parameter forces a copy of `my_path_buf`
even when the caller already owns a `PathBuf`.

```rust
fn push_path(list: &mut Vec<PathBuf>, item: &Path) {
    list.push(item.to_path_buf());
}
```

**Prefer:** the caller's `PathBuf` is moved in directly. A caller
with a `&Path` writes `push_path(&mut list, p.to_path_buf())`,
performing the same copy that the borrowed signature was
performing inside the function.

```rust
fn push_path(list: &mut Vec<PathBuf>, item: PathBuf) {
    list.push(item);
}
```

The total number of copies is the same in either signature for the
worst caller; for the best caller, the owned signature is one copy
cheaper.

## What to lint

For each function parameter typed as `&T` (or `&mut T`) where `T`
is one of the recognised owned-from-borrowed types
(`String`/`&str`, `PathBuf`/`&Path`, `OsString`/`&OsStr`,
`Vec<U>`/`&[U]`, `CString`/`&CStr`, plus any user-configured
extras), walk the function body looking for *unconditional*
conversion of the parameter to its owned form:

- `param.to_owned()`
- `param.to_string()` (when `T == str`)
- `param.to_path_buf()` (when `T == Path`)
- `param.to_vec()` (when `T == [U]`)
- `param.clone()` (when the receiver is `&T` and the result is `T`)
- `<T>::from(param)`, `T::from(param)` (when `T` is the owned form)
- `param.into()` (when the inferred target is `T`)

The conversion qualifies as *unconditional* if it dominates every
exit from the function — i.e., the conversion appears on every
control-flow path that uses the parameter. A conditional conversion
inside an `if` arm doesn't qualify; the owned form may not be
needed.

When the predicate holds, suggest changing the parameter type to
the owned form and removing the conversion call.

### Exemptions

The lint stays silent when:

- The function is a trait method whose signature is fixed by the
  trait. The implementer cannot change the parameter type.
- The parameter has lifetime constraints with other parameters
  (`fn f<'a>(x: &'a str, y: &'a str) -> &'a str`). Changing one to
  owned would break the lifetime contract.
- The parameter is *also* used in borrowed form elsewhere in the
  body (e.g., printed via `{:?}` and *then* converted). Owning the
  parameter changes nothing for those uses, but the lint stays
  conservative — the user can refactor manually.

## Examples

**Avoid:**

```rust
fn push_path(list: &mut Vec<PathBuf>, item: &Path) {
    list.push(item.to_path_buf());
}
```

**Prefer:**

```rust
fn push_path(list: &mut Vec<PathBuf>, item: PathBuf) {
    list.push(item);
}
```

**Avoid:**

```rust
fn store(name: &str, registry: &mut HashMap<String, u32>) {
    registry.insert(name.to_owned(), 0);
}
```

**Prefer:**

```rust
fn store(name: String, registry: &mut HashMap<String, u32>) {
    registry.insert(name, 0);
}
```

**Not flagged:** the conversion is conditional.

```rust
fn maybe_store(name: &str, registry: &mut HashMap<String, u32>) {
    if !registry.contains_key(name) {
        registry.insert(name.to_owned(), 0);
    }
}
```

**Not flagged:** the parameter is used in borrowed form too.

```rust
fn log_and_store(name: &str, registry: &mut HashMap<String, u32>) {
    eprintln!("inserting {name}");
    registry.insert(name.to_owned(), 0);
}
```

## Configuration

```toml
[prefer_owned_parameter]
# Pairs of (borrowed_type, owned_type) the lint recognises. The
# defaults cover the standard library's common cases; extend this
# for project-specific newtypes.
type_pairs = [
  { borrowed = "str",        owned = "String" },
  { borrowed = "Path",       owned = "PathBuf" },
  { borrowed = "OsStr",      owned = "OsString" },
  { borrowed = "CStr",       owned = "CString" },
  # `Vec<T>` / `[T]` is recognised generically; no entry needed.
]

# Methods that count as "conversion to owned form". Defaults cover
# the inherent conversions on the std types listed above.
conversion_methods = [
  "to_owned", "to_string", "to_path_buf", "to_vec", "clone", "into",
]
```

## Implementation notes

- `LateLintPass`. Walk every `ItemKind::Fn` and every
  `ImplItemKind::Fn` whose signature is *not* dictated by a trait
  (skip `tcx.trait_of_item(def_id).is_some()` for impl methods).
- For each parameter, check the type. If it is `&T` or `&mut T`
  where `T` matches an entry in `type_pairs` (or is `[U]` whose
  owned form is `Vec<U>`), record the parameter as a candidate.
- Walk the function body via a `Visitor`. For each candidate
  parameter:
  1. Track every reference: collect `Span`s and classify each as
     "conversion to owned" or "borrowed use".
  2. Run a control-flow dominance check: if every path from entry
     to a use of the parameter passes through a conversion to
     owned, the predicate holds.
  3. Confirm there is no "borrowed use" reference that is not
     itself the receiver of a conversion call.
- Suggested fix: rewrite the parameter type from `&T` to `T` and
  remove the trailing conversion call from each invocation.
  `Applicability::MaybeIncorrect` because removing the conversion
  alters the expression's type at the use site, which may cascade
  into other type-inference changes the lint cannot verify.

### Difficulty

**Medium.** The pattern is well-defined and local, but the
control-flow dominance check requires walking the HIR and tracking
which uses of the parameter are dominated by which conversion call.
A naive single-pass walk catches the easy cases (`fn f(x: &T) {
let y = x.to_owned(); ... }` where the conversion is the first
statement); the dominance-aware version handles branching bodies
correctly.

A conservative starting implementation:

- Fire only when the parameter is used *exactly once* in the body
  and that use is the conversion to owned. No dominance analysis
  needed; the predicate degenerates to a single-use check.
- Defer the multi-use / branching cases to a later pass.

## Default state

Active by default.

## Interaction with clippy

- `clippy::ptr_arg` flags `&Vec<U>`, `&String`, `&PathBuf`
  parameters and suggests the more permissive `&[U]`, `&str`,
  `&Path`. That covers the "use the most encompassing type" case
  from the pacquet guide and is orthogonal to this rule.
- `clippy::needless_pass_by_value` covers the *opposite* direction
  of this rule: a parameter taken by value but never moved should
  be taken by reference. Enable both clippy lints alongside
  `prefer_owned_parameter` for full coverage of the pacquet
  guide's owned-vs-borrowed trade-off.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
