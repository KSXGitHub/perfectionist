# `unmarked_test_module`

**Source:** project convention, prompted by
<https://github.com/HoangVanKhai/my-translated-lyrics/pull/86#discussion_r3448005837>.
When a long `tests.rs` is split into a flat layout of separate-file
submodules, an AI assistant tends to name the new modules after the
*subject under test* (`buttons`, `mouse`, `rendering`) and gate each
with `#[cfg(test)]`. Read from the file tree alone, those names are
indistinguishable from production submodules.

## Statement

A submodule that is compiled only under `cfg(test)` should carry a
test marker in its **name**, so that the name alone — in a `mod`
declaration or in the file tree — identifies it as test code.

The marker is the word `test` as a leading or trailing `_`-delimited
segment:

- a **prefix** word — `test_buttons`, `test_list_keyboard`; or
- a **suffix** word — `buttons_tests`, `list_keyboard_tests`
  (`_test` singular is accepted too).

The conventional standalone names `tests` and `test` already satisfy
this — their only segment *is* the marker — so the ubiquitous
`#[cfg(test)] mod tests { ... }` is never flagged.

## What to lint

Flag a `mod` item — inline `mod foo { ... }` or out-of-line
`mod foo;` — when **both** hold:

1. The module is compiled **only** under test. Its own attribute list
   carries a `cfg` whose predicate makes `test` mandatory: bare
   `#[cfg(test)]`, or `#[cfg(all(test, ...))]` where `test` is a
   conjunct. A predicate that leaves the module compiled outside test
   too — `#[cfg(any(test, feature = "x"))]`, `#[cfg(not(test))]` — is
   **not** a test-only module and is out of scope.
2. The module's name carries no test marker: splitting the name on
   `_`, neither the first segment is `test` nor the last segment is
   `test` / `tests`.

Matching is on whole `_`-delimited segments, never substrings, so a
production-spelled name that merely *contains* the letters
(`latest`, `contest`) is not mistaken for a marked one — and such a
module would have to be `cfg(test)`-gated to be considered at all.

## Examples

**Avoid:** a test-only submodule named like a production module

```rust
// selectors.rs
#[cfg(test)] mod buttons;
#[cfg(test)] mod list_keyboard;
#[cfg(test)] mod mouse;
```

```text
selectors.rs
selectors/buttons.rs
selectors/list_keyboard.rs
selectors/mouse.rs
```

**Prefer:** the `test_` prefix form

```rust
#[cfg(test)] mod test_buttons;
#[cfg(test)] mod test_list_keyboard;
#[cfg(test)] mod test_mouse;
```

**Prefer:** the `_tests` suffix form

```rust
#[cfg(test)] mod buttons_tests;
#[cfg(test)] mod list_keyboard_tests;
#[cfg(test)] mod mouse_tests;
```

**Not flagged:** the conventional inline test module

```rust
#[cfg(test)]
mod tests {
    // ...
}
```

**Not flagged:** a module that is *not* test-only

```rust
#[cfg(any(test, feature = "mock"))]
mod buttons;          // also compiled with `mock`; a production module
```

## Configuration

```toml
[perfectionist::unmarked_test_module]
# Which marker form the autofix suggests. Defaults to "prefix".
# Both forms are *accepted* regardless of this value; `style` only
# selects the shape of the suggested rename.
style = "prefix"
# "prefix" — suggest `test_<name>`.
# "suffix" — suggest `<name>_tests`.
```

The rule accepts either marker; `style` is purely about which rewrite
the diagnostic offers. A project that wants every test module spelled
one specific way relies on the suggested form being consistent rather
than on the rule rejecting the other marker — enforcing a *single*
marker form (flagging `foo_tests` under a prefix-only policy) is left
out of scope; the trigger here is the absence of *any* marker.

## Autofix

The diagnostic suggests renaming the module identifier to the
`style`-selected form (`buttons` → `test_buttons` or `buttons_tests`).

The fix is **`MaybeIncorrect`**, never machine-applicable, for two
reasons:

1. For an out-of-line `mod foo;`, the backing file must be renamed in
   lockstep (`selectors/buttons.rs` → `selectors/test_buttons.rs`),
   which a source rewrite cannot perform. The help text must say so
   explicitly.
2. Any `path::to::buttons::...` reference from elsewhere (rare for
   test-only modules, but possible from sibling test code) would need
   updating too.

## Implementation notes

This rule reads the **written layout** of items across module scopes,
so it follows the source-layout discipline in the *"Reaching every
module"* section of
[`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
run as a `LateLintPass` and re-parse the crate's module files through
`src/module_reparse.rs` rather than walking the AST in an
`EarlyLintPass`. An `EarlyLintPass` would be wrong twice over here —
pre-expansion it leaves every out-of-line `mod foo;` as
`ModKind::Unloaded` (the recurring source-layout trap), and the rule's
whole subject, `#[cfg(test)]`-gated declarations, is cfg-stripped out
of the post-expansion HIR in a normal (non-test) `cargo dylint` build.
Re-parsing preserves the `#[cfg(test)]` attribute the rule keys on.

- The trigger is the `mod` **declaration** item, found among a module
  file's top-level items; the rule does not need to descend *into* the
  test module's body. So it sees `#[cfg(test)] mod buttons;` declared
  in the live parent file `selectors.rs` without recursing into a
  cfg-disabled body — the `live_module_spans` caveat about descending
  into cfg-disabled inline modules does not bite the common case.
- Anchor each violation at its enclosing HIR node via
  `enclosing_hir::find_enclosing_hir_ids`, emitting through
  `clippy_utils::diagnostics::span_lint_hir_and_then`, so a
  `#[allow(perfectionist::unmarked_test_module)]` on the parent module
  (or crate root) resolves. The diagnostic span is the `mod` item's
  name identifier.
- Detecting the `cfg(test)` gate is a small fixed walk of the item's
  `cfg` attribute predicate (is `test` a mandatory conjunct?), not a
  string scan — it does not need the parser-combinator scaffold.
- The name check is a trivial segment split, not a parser-combinator
  scan: take the identifier, split on `_`, compare the first segment
  to `test` and the last to `test` / `tests`. (See *"Where to draw the
  line"* in the conventions file.)

- **Limitation (over-approximation by omission).** A
  `#[cfg(test)] mod foo;` declared *inside* another test-only module
  whose file is itself cfg-gated out of a non-test build is not
  reached, because that file does not back a live HIR module and so is
  absent from the re-parsed file set. The issue's motivating case — one
  level of test submodules declared in a production parent file — is
  fully covered.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Why restrict this?

This is a stylistic preference, not a correctness issue: the code
compiles and the tests run identically whatever the module is named.
The preference is about *reading the crate's structure*. A flat
test-module layout —

```text
selectors.rs
selectors/buttons.rs
selectors/mouse.rs
```

— gives a reader scanning the file tree no signal that `buttons.rs`
and `mouse.rs` are test fixtures rather than production submodules of
`selectors`. The `#[cfg(test)]` gate lives in the parent file's `mod`
line, not in the file name, so the distinction is invisible until you
open each file. A marker in the name restores it at a glance, in both
the file tree and the list of `mod` declarations.

## Suppressing proc-macro-synthesised violations

A `mod` declaration is not a node a derive macro realistically
synthesises with a user-source span, and the diagnostic span covers
the whole construct's name rather than a borrowed inner token, so the
*"vulnerable exactly when"* test in the conventions file clears this
rule: no proc-macro guard or `ui/unmarked_test_module_proc_macro.rs`
fixture is required. Record the omission as deliberate at the
span-selection site.

## Interaction with sibling lints

[`cfg-attr-ignore-tests`](./cfg-attr-ignore-tests.md) also keys on
`#[cfg(test)]` / `#[cfg_attr(...)]`
attributes, but on `#[test]` *functions* and their skip mechanism, not
on module naming; the two never fire on the same node. The
module-file-reaching machinery (`src/module_reparse.rs`) is shared with
`perfectionist::import_granularity_mismatch`,
`perfectionist::import_grouping_mismatch`, and
`perfectionist::uncombined_self_import`.

Integration tests under `tests/` are **out of scope**: each is its own
crate root reached as a file, not a `cfg(test)`-gated `mod` declaration
inside the library or binary crate, so the trigger never applies to
them.

## Default state

Active by default. The baseline — *a test-only module must carry some
test marker* — is not presumptuous about which marker (it accepts both
the prefix and suffix forms), and the conventional `tests` / `test`
names are exempt, so the rule stays quiet on idiomatic code.
