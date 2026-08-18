# `path_qualification_mismatch`

**Source:** project convention; parallel-disk-usage's codebase
implicitly enforces the unqualified form by always importing items
before use. AI assistants — particularly those producing
single-snippet answers without context for surrounding `use`
declarations — strongly prefer the qualified form, which is the
behaviour this rule addresses.

## Statement

When code names an item from outside the current scope, the
*qualification depth* of the path is a project-style decision:

- **Unqualified**: the item is imported via `use`, then named by
  its simple identifier:
  ```rust
  use std::fs::create_dir_all;
  use clap::Parser;

  create_dir_all(&dir).unwrap();
  #[derive(Parser)]
  struct Cli;
  ```
- **Qualified**: the item is named through one or more path
  segments at every call site, with no `use` for the leaf item:
  ```rust
  std::fs::create_dir_all(&dir).unwrap();
  #[derive(clap::Parser)]
  struct Cli;
  ```

Both compile. The choice is purely stylistic and the rule lets a
project enforce one consistently.

The rule does **not** distinguish between *fully* qualified
(`std::fs::create_dir_all`) and *module-qualified*
(`fs::create_dir_all` with `use std::fs;`) — both count as
"qualified" because both contain at least one path segment before
the leaf identifier. A project that wants finer control can list
specific path prefixes in `allowed_paths` / `forbidden_paths`.

## What's *not* in scope

The rule treats paths whose first segment is a **type** rather than
a module as exempt from the `unqualified` style — `String::new()`,
`HashMap::with_capacity(64)`, `Vec::<u8>::new()`, and
`Iterator::map(it, f)` all stay as written. The qualification
through a type is functionally a method call, not a navigation
through namespaces.

Self-referential prefixes (`Self`, `self`, `super`, `crate`) are
also exempt. They name the path's *origin* rather than naming an
external module.

## Configuration

```toml
[path_qualification_mismatch]
# Inactive by default. Enable in `[perfectionist].enable`, then set
# `style` — it is mandatory and has no default. The value below is an
# example, not a default.
style = "unqualified"
# "unqualified"  — flag any module-qualified path; suggest a `use`
#                  for the leaf and replace with the simple ident.
# "qualified"    — flag any simple-ident path that resolves to an
#                  item from outside the current module; suggest
#                  the full path and removal of the `use`.

# Where the lint applies. Each context can be toggled independently.
# Deliberately an array, not per-context bools: at this size it
# still reads cleanly, and it's a documented exception to the
# boolean-fields convention (see IMPLEMENTATION_CONVENTIONS.md).
contexts = ["call", "type", "derive", "macro", "trait_bound"]

# Prefixes that the `unqualified` style leaves alone. Defaults to
# self-referential prefixes plus `Self::`-style associated calls.
unqualified_skip_prefixes = ["Self", "self", "super", "crate"]

# Whether `unqualified` style also exempts type-associated calls
# (`String::new`, `Vec::<u8>::with_capacity`). Defaults to true; a
# project can turn off the type exemption to push for free-function
# imports of associated constructors.
unqualified_skip_type_assoc = true

# Under `qualified` style, items defined in the *same* module are
# not flagged. Defaults to true.
qualified_skip_local_module = true

# Paths that the lint never flags, regardless of style.
allowed_paths = [
  # e.g. always-qualified to avoid colliding with std::mem::replace
  # "crate::shadowed::replace",
]

# Paths that the lint always flags in the configured direction.
forbidden_paths = []
```

## Style: `unqualified`

**Avoid:**

```rust
std::fs::create_dir_all(&dir).unwrap();
#[derive(clap::Parser)]
struct Cli;
let parsed: serde_json::Value = serde_json::from_str(&s)?;
```

**Prefer:**

```rust
use clap::Parser;
use serde_json::Value;
use std::fs::create_dir_all;

create_dir_all(&dir).unwrap();
#[derive(Parser)]
struct Cli;
let parsed: Value = serde_json::from_str(&s)?;   // function name
                                                  // imported separately
```

The autofix:

1. Adds a `use` for the leaf identifier (or merges into an existing
   `use` per `perfectionist::import_granularity_mismatch`).
2. Replaces the qualified site with the leaf ident.

The fix is `MachineApplicable` only when adding the `use` does not
introduce a name collision — see *Same-name conflicts* below. When
a collision is possible the lint emits help text and does not
auto-rewrite.

## Style: `qualified`

**Avoid:**

```rust
use clap::Parser;
use std::fs::create_dir_all;

create_dir_all(&dir).unwrap();
#[derive(Parser)]
struct Cli;
```

**Prefer:**

```rust
std::fs::create_dir_all(&dir).unwrap();
#[derive(clap::Parser)]
struct Cli;
```

The autofix substitutes the simple-ident site with the full path
and removes the `use` declaration once it is no longer needed.
The lint is `MaybeIncorrect` here because the canonical full path
may not be the only valid path for the item (re-exports, inherent
glob imports, prelude items) — the fix picks the path that
`tcx.def_path` reports, which matches the item's *definition*.

## Same-name conflicts

The `unqualified` style cannot fire when adding a bare `use` would
collide with an existing identifier in scope:

```rust
struct Path { /* local Path */ }

fn process(p: &Path) {
    std::path::Path::new("/").exists();   // staying qualified is correct
}
```

A `use std::path::Path;` here would shadow or collide with the
local `Path`. The lint must detect this and stay silent.

Detection: resolve the path's leaf to its `DefId`, then ask the
resolver whether that simple name is *already* bound in the
target scope to a different `DefId`. If yes, this site is exempt.

Same-name conflicts can also be project-wide: two crates exporting
`Error` make `use foo::Error; use bar::Error;` impossible. The lint
should fire only on one of them at a time, leaving the project
owner to qualify the other.

## What to lint

Walk every path in the configured `contexts` set:

- **`call`**: `ExprKind::Path` followed by `ExprKind::Call`, and
  `ExprKind::MethodCall` (for trait-method-via-UFCS).
- **`type`**: `TyKind::Path` in declarations, generic args, and
  `let` annotations.
- **`derive`**: paths inside `#[derive(...)]` argument lists.
- **`macro`**: macro invocation paths
  (`my_crate::my_macro!(...)`).
- **`trait_bound`**: paths inside `where` clauses and inline trait
  bounds.

For each path:

1. Discard paths starting with a configured `unqualified_skip_prefixes`
   entry.
2. Resolve the leaf to its `DefId`. If `tcx.parent(def_id)` is a
   `Type`-namespace item (struct, enum, trait), apply the
   `unqualified_skip_type_assoc` rule.
3. Apply the configured `style`:
   - `unqualified`: flag if the path has more than one segment and
     none of the exemptions apply, *and* a same-name collision is
     not present.
   - `qualified`: flag if the path has exactly one segment, the
     item is defined outside the current module, and
     `qualified_skip_local_module` does not exempt it.
4. Cross-check against `allowed_paths` (always pass) and
   `forbidden_paths` (always flag in the configured direction).

## Implementation notes

- `LateLintPass`. The same-name-collision check requires the
  resolver, which is only available in late pass.
- For `unqualified`, the lint must coordinate with the `use` block
  in the enclosing module: the suggestion adds an import, which
  may interact with `perfectionist::import_granularity_mismatch`
  and `perfectionist::import_grouping_mismatch`. When all three
  rules are enabled, a `cargo clippy --fix` pass should run
  iteratively until fixed-point — the diagnostic from this rule
  emits the new `use` line in its raw form (one per leaf), and
  the import lints reflow the result.
- For `qualified`, the canonical path comes from `tcx.def_path` —
  but be aware that the *definition* path may not be the path the
  user is expected to use. `Vec` is defined in `alloc::vec::Vec`
  but most projects refer to it as `std::vec::Vec` or just `Vec`.
  The lint's path resolution should respect the
  [`core-instead-of-std`](./core-instead-of-std.md) preference if it is
  active, so a suggested path uses `std::` rather than the
  `core` / `alloc` definition path.
- **Parser style.** The configuration parser
  (`allowed_paths`, `forbidden_paths`) takes path strings; parse
  them with parser-combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md).
  Path-string equality must agree with rustc's: two paths match if
  their segment sequences are equal *after* resolving leading
  `crate::` / `self::` to absolute paths in the project's context.

### Difficulty

**Medium-hard.**

- The body-pattern detection is straightforward: count path
  segments, classify the leaf, and consult the resolver.
- The same-name collision check is the wedge. False positives
  (lint suggests an import that would shadow a local) are
  unacceptable; conservative detection that asks the resolver
  before each suggestion keeps the false positive rate at zero
  but slows the lint slightly. Both are acceptable trade-offs.
- The `qualified` direction's autofix has the
  canonical-vs-preferred-path problem described above. Handle by
  consulting [`core-instead-of-std`](./core-instead-of-std.md) config when
  present.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Interaction with `clippy::absolute_paths`

`clippy::absolute_paths` (a `restriction` lint, off by default) is the
closest stock lint, but the two are not interchangeable. It flags a
path with more than `absolute-paths-max-segments` leading segments and
emits a bare warning with no autofix — i.e. it only ever argues
*against* long *absolute* paths (anchored at a crate root), with a
length threshold rather than a project-wide direction. So it is silent
on the module-qualified relative path this rule most wants to catch —
`fs::create_dir_all(&dir)` with `use std::fs;` in scope — and has no
equivalent of the `qualified` direction at all. This rule instead
enforces one chosen direction everywhere (`unqualified` *or*
`qualified`), independent of length. The two overlap only in the
`unqualified` direction and agree only on the fully-qualified variant;
a project that just wants the "shorten long absolute paths" heuristic
can use `clippy::absolute_paths` instead.

## Default state

Inactive by default. `unqualified` vs. `qualified` is a per-project
preference, so the rule ships no baseline; enable it in
`[perfectionist].enable` and set `style`. `style` is mandatory once
enabled — see
[Mandatory configuration on opt-in rules](./IMPLEMENTATION_CONVENTIONS.md#mandatory-configuration-on-opt-in-rules).

## Why one rule instead of two

`forbid-path-qualification-mismatch` and `require-path-qualification-mismatch` describe the
same axis from opposite ends. Two separate rules would have to
coordinate so they never both fire on the same path (each
direction's "good" form is the other's "bad" form). One rule with
a `style` knob keeps the policy expressible in one place — the
same shape as
`perfectionist::import_granularity_mismatch`,
`perfectionist::unordered_derives`, and
[`serde-wrapper-form-mismatch`](./serde-wrapper-form-mismatch.md).
