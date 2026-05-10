# `unknown_perfectionist_lints`

**Source:** project convention. Complements the consumer-side
caveat described in
[`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
under "Caveat: the consumer-side `unknown_lints` warning". Typos,
renames, and stale references in
`#[allow(perfectionist::...)]` attributes silently neutralise the
suppression they were written for; this rule turns the silent
failure into a warning that names what went wrong.

## Statement

Inside a lint-control attribute (`allow`, `warn`, `deny`, `forbid`,
`expect`, or any of those nested in `cfg_attr`), a lint name whose
first path segment is `perfectionist` must resolve to a lint
actually registered by this plugin. Names that don't match any
registered lint are flagged.

```rust
// Bad — typo: `qualified_paths`, not `qualified_path`.
#[allow(perfectionist::qualified_path)]
fn legacy() { /* ... */ }

// Good
#[allow(perfectionist::qualified_paths)]
fn legacy() { /* ... */ }
```

The rule fires only on names under the `perfectionist` tool
prefix. Bare names (`#[allow(qualified_paths)]`) and other tool
namespaces (`#[allow(clippy::...)]`, `#[allow(rustdoc::...)]`)
are out of scope — rustc's built-in `unknown_lints` already
handles those, and this plugin has no opinion about other tools'
names.

## What to lint

Walk every attribute whose path is one of:

- `allow`, `warn`, `deny`, `forbid`, `expect`
- `cfg_attr(<predicate>, <inner-attr>)` where `<inner-attr>` is
  one of the above; descend exactly once into the inner attribute.

For each name argument inside that meta list whose first segment
is `perfectionist`:

1. Treat the segment(s) after the prefix as the candidate lint
   name.
2. Look the candidate up in the plugin's registered-lint set (the
   canonical list of `declare_tool_lint!` identifiers, lowercased
   to snake_case).
3. If absent, emit a warning that names the unknown lint and,
   when a near match exists in the registered set, suggests it.

Names with **zero** trailing segments (`perfectionist`) and names
with **two or more** trailing segments
(`perfectionist::imports::granularity`) are both unknown — the
plugin registers exactly one segment after the tool prefix. Lint
groups, if the plugin ever registers any, count as registered
names alongside individual lints.

## Examples

```rust
// Bad — lint was renamed during implementation
#[allow(perfectionist::no_glob_imports)]
fn used_to_work() {}

// help: did you mean `perfectionist::no_star_imports`?
```

```rust
// Bad — typo
#[deny(perfectionist::single_letter_name)]
fn x() {}

// help: did you mean `perfectionist::single_letter_names`?
```

```rust
// Bad — depth mismatch
#[allow(perfectionist::imports::granularity)]
mod x {}

// help: did you mean `perfectionist::import_granularity`?
```

```rust
// Bad — tool prefix with no lint name
#[allow(perfectionist)]
fn no_target() {}
```

```rust
// Allowed — different tool namespace
#[allow(clippy::needless_return)]
fn other_tool() {}

// Allowed — bare lint name; rustc's `unknown_lints` owns this case.
#[allow(qualified_paths)]
fn untouched() {}

// Allowed — `cfg_attr` wrapping a registered name
#[cfg_attr(test, allow(perfectionist::single_letter_names))]
fn t() {}
```

## Configuration

```toml
[unknown_perfectionist_lints]
# Edit-distance threshold for the "did you mean" hint, measured
# against the part after `perfectionist::`. 0 disables suggestions.
suggestion_distance = 2

# Extra names the lint should treat as registered. Useful while
# migrating away from a renamed lint: each entry silences the
# warning at every call site without forcing every consumer to
# scrub the old attribute in a single PR.
extra_known_names = []
```

## Implementation notes

- `EarlyLintPass::check_attribute`. Lint-control attributes and
  `cfg_attr` are both visible at the AST level, well before
  name resolution runs.
- The registered-name set is exactly the list of lints the
  plugin's `register_lints` callback passes to Dylint. Expose it
  as a `&'static [&'static str]` built from each rule module's
  `LintPass::name()` call, stripped of the `perfectionist::`
  prefix; or query the lint store directly via
  `LintStore::get_lints().iter().filter(|l| l.name.starts_with("perfectionist::"))`.
- For the "did you mean" hint, use straightforward Levenshtein
  edit distance on the candidate name against the registered set.
  The set has tens of entries; a quadratic scan per attribute is
  fine.
- `cfg_attr` descent: exactly one level. Nested
  `cfg_attr(cfg_attr(..., x))` is malformed and rustc rejects it
  before this pass runs.
- The lint must itself be registered, so it can be silenced by
  `#[allow(perfectionist::unknown_perfectionist_lints)]` — the
  attribute name resolves and the lint stays silent on its own
  suppression site.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Interaction with rustc's `unknown_lints`

`unknown_lints` is rustc's built-in lint for the same family of
problem. It fires inconsistently on tool-prefixed names — see the
caveat in
[`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
— which is precisely the case this rule covers. When both fire
the diagnostics are redundant but harmless; this rule's message
is more specific (it knows the registered set) and includes a
"did you mean" hint that rustc's generic implementation does not
produce for tool-namespaced names.

The rule does **not** suppress `unknown_lints`; users who want
the warning emitted only once can `#[allow(unknown_lints)]` at
the crate root, as the conventions document recommends for the
cross-toolchain case.

## Out of scope

- **`dylint.toml` keys.** A misspelled
  `[perfectionist::unknown_lint]` table never reaches the lint
  pass (it is consulted by configuration loading, not by source
  attributes). Validating the config table is a separate concern,
  better handled by a startup check inside the plugin's
  configuration parser.
- **`RUSTFLAGS="-A perfectionist::foo"`** and equivalent
  command-line lint controls. They are outside source and outside
  the AST.

## Difficulty

**Easy.**

- The attribute walk is shallow and the AST API exposes every
  case directly.
- The registered-name set is already enumerable inside the plugin.
- The only judgement call is the suggestion-distance default, and
  that is configurable.

## Severity

Warn. The original lint the user meant to suppress still fires
elsewhere; the user is told twice rather than missing both
diagnostics.
