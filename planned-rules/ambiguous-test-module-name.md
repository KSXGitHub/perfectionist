# `ambiguous_test_module_name`

**Source:** project convention, prompted by
<https://github.com/HoangVanKhai/my-translated-lyrics/pull/86#discussion_r3448005837>.
When a long `tests.rs` is split into a flat layout of separate-file
submodules, an AI assistant tends to name the new modules after the
*subject under test* (`buttons`, `mouse`, `rendering`) and gate each
with `#[cfg(test)]`. Read from the file tree alone, those names are
ambiguous — indistinguishable from production submodules.

## Statement

A submodule that is compiled only under `cfg(test)` should be named so
that the name alone — in a `mod` declaration or in the file tree —
identifies it as test code. Its name must match one of the project's
recognized test-name patterns: an exact name (`tests`, `spec`), a
prefix (`test_…`), or a suffix (`…_tests`). A name that matches none of
them is ambiguous, and the rule flags it.

The recognized patterns are fully configurable; the defaults cover the
common `test` / `testing` / `spec` conventions in both their whole-word
and affixed forms.

## What to lint

Flag a `mod` item — inline `mod foo { ... }` or out-of-line
`mod foo;` — when **both** hold:

1. The module is compiled **only** under test — that is, switching
   `test` *off* would switch the module off. Equivalently, `test` is a
   mandatory conjunct of the module's `cfg` predicate: bare
   `#[cfg(test)]`, or an `all(...)` with `test` anywhere among its
   operands (`#[cfg(all(unix, test))]` counts exactly as
   `#[cfg(all(test, unix))]` does — conjunct order is irrelevant). A
   predicate that leaves the module compiled outside test too is **not**
   a test-only module and is out of scope: `#[cfg(any(unix, test))]`
   still compiles on `unix` without `test`, and `#[cfg(not(test))]` /
   `#[cfg(feature = "x")]` are not test-gated at all.
2. The module's name matches **none** of the configured patterns: it is
   not equal to any `whole_names` entry, does not start with any
   `prefixes` entry, and does not end with any `suffixes` entry.

Matching is literal string equality / prefix / suffix. Each affix entry
carries its own separator (`test_`, `_tests`), so the boundary is
explicit and the rule does no segment-splitting or substring guessing:
a production-spelled name that merely *contains* the letters (`latest`,
`contest`) does not start with `test_` or end with `_test`, so it is
unaffected — and such a module would have to be `cfg(test)`-gated to be
considered at all.

## Configuration

```toml
[perfectionist::ambiguous_test_module_name]
# A name accepted when it equals the whole module name.
# Allows `tests.rs`, `spec.rs`, etc.
whole_names = ["test", "testing", "tests", "spec", "specs"]
# A name accepted when the module name starts with it (separator
# included). Allows `test_buttons`, `spec_buttons`, etc.
prefixes = ["test_", "testing_", "spec_"]
# A name accepted when the module name ends with it (separator
# included). Allows `buttons_test`, `buttons_specs`, etc.
suffixes = ["_test", "_testing", "_tests", "_spec", "_specs"]
```

Each field is an open-ended list of user strings, so all three stay
arrays (per the *"Config shape"* section of
[`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md) — an
open-ended list is one of the shapes that is *not* the boolean-toggle
anti-pattern). A test-only module is accepted when its name matches
**any** entry across the three lists. Because the affixes include their
own separator, a project that wants a bare-word prefix (`testbuttons`)
or a different separator (`test-` is not a valid identifier, but
`test2_` would be) controls it entirely through the entry text. An
empty list disables that matching mode; emptying all three makes every
test-only module ambiguous, so the defaults are the intended baseline.

## Examples

**Avoid:** a test-only submodule whose name matches no pattern

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

**Prefer:** a `prefixes` match

```rust
#[cfg(test)] mod test_buttons;
#[cfg(test)] mod test_list_keyboard;
#[cfg(test)] mod test_mouse;
```

**Prefer:** a `suffixes` match

```rust
#[cfg(test)] mod buttons_tests;
#[cfg(test)] mod list_keyboard_tests;
#[cfg(test)] mod mouse_tests;
```

**Not flagged:** a `whole_names` match (the conventional inline module)

```rust
#[cfg(test)]
mod tests {
    // ...
}
```

**Avoid:** test-only via `all(...)` (conjunct order irrelevant)

```rust
#[cfg(all(unix, test))]
mod buttons;          // off when `test` is off ⇒ test-only ⇒ flagged
```

**Not flagged:** a module that is *not* test-only

```rust
#[cfg(any(unix, test))]
mod buttons;          // still compiled on `unix` without `test`;
                      // a production module, so out of scope

#[cfg(any(test, feature = "mock"))]
mod widgets;          // likewise also compiled with `mock`
```

**Not flagged:** a test submodule nested below a *complying* boundary

```text
selectors.rs           #[cfg(test)] mod tests;   // complies, checked
selectors/tests.rs     mod buttons;              // not checked (see Scope)
selectors/tests/buttons.rs
```

`buttons` already sits under a test-named path (`selectors/tests/...`),
so it is not ambiguous and is left alone.

## Scope: only the outermost test-only module

The rule checks the **outermost** test-only `mod` in any chain — the
declaration at the boundary between production and test code — and not
test submodules nested *below* it. This is by design, and the file tree
shows why:

```text
selectors.rs                 #[cfg(test)] mod tests;   ← checked (complies)
selectors/tests.rs           mod buttons;              ← not checked
selectors/tests/buttons.rs
```

Because `buttons` is declared inside `tests.rs`, a file backing a
`#[cfg(test)]` module, its whole subtree already lives under a
test-named path. The `tests/` component *is* the test signal, so
`buttons.rs` under it carries no ambiguity and there is nothing to
flag. The same holds when the boundary module *violates*: flagging
`selectors::helpers` and leaving `helpers::buttons` alone is the right
granularity, since renaming `helpers` → `test_helpers` disambiguates
the entire subtree in one move.

Mechanically this falls out of the source-layout machinery for free.
The re-parse reaches only files that back a *live* HIR module, and in a
normal (non-test) `cargo dylint` build a `#[cfg(test)]` module's file
is not live — so its contents are never walked. The outermost test-only
`mod`, by contrast, always sits in a live parent file (the production
module that declares it) and is always reached. So the one place
ambiguity can arise — the production↔test boundary — is exactly the
place the rule inspects.

**The boundary is defined by `#[cfg(test)]`, not by the name.**
"Outermost test-only" means the first `#[cfg(test)]` gate encountered
descending from the crate root, whatever the intervening modules are
*named*:

```text
foo::bar::baz::qux      // only `qux` gated ⇒ `qux` is the boundary ⇒ flagged
foo::bar::tests::qux    // `tests` is a *production* module merely named
                        // `tests` (no gate); `qux` is gated ⇒ `qux` is the
                        // boundary ⇒ flagged
```

In the second case the path *contains* `tests`, but that module is
production code (it has no `#[cfg(test)]`), so it neither establishes a
boundary nor places its subtree "under tests". `qux` is the real
boundary and must announce itself. This is the exact contrast with the
`selectors::tests::buttons` example above, where `tests` genuinely
carries `#[cfg(test)]` and so really *is* the boundary — there `buttons`
is correctly left alone, here `qux` is correctly flagged. A test-named
but non-test-gated module never counts as a complying ancestor.

## Autofix

The diagnostic suggests renaming the module identifier so its name
matches a configured pattern. When `prefixes` is non-empty it offers
`<first prefix><name>` (e.g. `buttons` → `test_buttons`); when
`suffixes` is non-empty it offers `<name><first suffix>` (e.g.
`buttons` → `buttons_test`). The user picks; an IDE shows both.

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
  `#[allow(perfectionist::ambiguous_test_module_name)]` on the parent
  module (or crate root) resolves. The diagnostic span is the `mod`
  item's name identifier.
- Detecting the `cfg(test)` gate is a small fixed walk of the item's
  `cfg` attribute predicate (is `test` a mandatory conjunct?), not a
  string scan — it does not need the parser-combinator scaffold.
- The name check is three literal comparisons against the config lists
  (equality, `str::starts_with`, `str::ends_with`), not a
  parser-combinator scan. (See *"Where to draw the line"* in the
  conventions file.)

- Walking only live module files means test submodules nested below a
  test-only boundary are not reached — which is the intended scope, not
  a gap. See *"Scope: only the outermost test-only module"* above for
  why the boundary is the only place ambiguity arises.

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
open each file. A name matching a recognized test pattern restores it
at a glance, in both the file tree and the list of `mod` declarations.

## Suppressing proc-macro-synthesised violations

A `mod` declaration is not a node a derive macro realistically
synthesises with a user-source span, and the diagnostic span covers
the whole construct's name rather than a borrowed inner token, so the
*"vulnerable exactly when"* test in the conventions file clears this
rule: no proc-macro guard or `ui/ambiguous_test_module_name_proc_macro.rs`
fixture is required. Record the omission as deliberate at the
span-selection site.

## Interaction with sibling lints

[`cfg-attr-ignore-tests`](./cfg-attr-ignore-tests.md) also keys on
`#[cfg(test)]` / `#[cfg_attr(...)]` attributes, but on `#[test]`
*functions* and their skip mechanism, not on module naming; the two
never fire on the same node. The module-file-reaching machinery
(`src/module_reparse.rs`) is shared with
`perfectionist::import_granularity_mismatch`,
`perfectionist::import_grouping_mismatch`, and
`perfectionist::uncombined_self_import`.

Integration tests under `tests/` are **out of scope**: each is its own
crate root reached as a file, not a `cfg(test)`-gated `mod` declaration
inside the library or binary crate, so the trigger never applies to
them.

## Default state

Active by default. The baseline — *a test-only module's name must match
a recognized test-name pattern* — is not presumptuous: the default
lists accept the whole-word, prefix, and suffix forms of the common
`test` / `testing` / `spec` conventions, so the rule stays quiet on
idiomatic code and a project with a different convention adjusts the
three lists.
