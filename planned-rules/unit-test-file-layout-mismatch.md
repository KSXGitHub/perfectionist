# `unit_test_file_layout_mismatch`

**Source:** project convention; follows on from
`perfectionist::excessive_inline_tests`
(`src/rules/excessive_inline_tests.rs`). That rule pushes a large
inline `#[cfg(test)] mod tests { ... }` out into a separate
`foo/tests.rs` file; once that extracted file itself grows too big
and is split across several files, projects disagree about how the
resulting files should be arranged. This rule lets a project pick
one arrangement and enforce it.

## Statement

When the unit tests for a single subject file are split across
**more than one** out-of-line file, the *filesystem layout* of
those files is a project-style decision. Two arrangements are in
common use:

- **Siblings**: the subject file declares each test group directly
  as a sibling `#[cfg(test)] mod <group>;`, and the group files
  live next to one another under the subject's directory:

  ```text
  test_subject.rs
  test_subject/group_1.rs
  test_subject/group_2.rs
  test_subject/group_3.rs
  ```

  ```rust
  // test_subject.rs
  #[cfg(test)] mod group_1;
  #[cfg(test)] mod group_2;
  #[cfg(test)] mod group_3;
  ```

- **Aggregated**: the subject file declares a single
  `#[cfg(test)] mod tests;`, an intermediate aggregator file
  (`test_subject/tests.rs`) re-declares the groups with plain
  `mod <group>;` (no `#[cfg(test)]` — it is already inside a
  test-gated module), and the group files live one level deeper:

  ```text
  test_subject.rs
  test_subject/tests.rs
  test_subject/tests/group_1.rs
  test_subject/tests/group_2.rs
  test_subject/tests/group_3.rs
  ```

  ```rust
  // test_subject.rs
  #[cfg(test)]
  mod tests;
  ```

  ```rust
  // test_subject/tests.rs
  mod group_1;
  mod group_2;
  mod group_3;
  ```

Both compile and run identically. The choice is purely stylistic:
*siblings* keeps the directory flat at the cost of one
`#[cfg(test)]` per group in the production file; *aggregated* keeps
the production file to a single test-module line at the cost of an
extra aggregator file and a deeper directory. The rule lets a
project enforce one consistently and flags the other.

A test-gate predicate is recognised exactly as
`perfectionist::excessive_inline_tests` recognises it: a bare
`#[cfg(test)]` and any compound predicate that still implies test —
`#[cfg(all(test, unix))]`, `#[cfg(all(test, feature = "..."))]` —
count as test gates. A predicate that does *not* imply test, such as
`#[cfg(any(test, feature = "..."))]` (active outside test too), is
not a test gate, matching `excessive_inline_tests`'s own
`any(test, …)` handling. The aggregator's inner `mod <group>;`
declarations need no gate of their own because the module they sit
in is already test-gated.

## What's *not* in scope

- **A single extracted test file.** A subject with exactly one
  out-of-line test module (`#[cfg(test)] mod tests;` →
  `test_subject/tests.rs` holding the test code) is the extraction
  target of `perfectionist::excessive_inline_tests`, not a
  multi-file split. Neither style fires below the `min_files`
  threshold; with zero or one test file there is no layout to
  choose.
- **Group file *names*.** The rule normalises *where* the group
  files sit and *how* they are declared, never what they are
  called. A project is free to name groups `group_1` / `parsing` /
  `regression` as it likes; renaming is the author's job.
- **Production submodules.** A subject file may also declare
  non-test out-of-line modules (`mod helpers;`). Only modules
  reached through a test gate participate; production modules are
  untouched.
- **Deeper nesting.** The rule normalises exactly one level of
  split — the subject and its immediate test groups. A project that
  nests groups further (`test_subject/tests/parsing/edge.rs`) is
  beyond the two documented layouts and is left alone.
- **Inline test code in the subject file.** Whether test code
  belongs inline or extracted at all is
  `perfectionist::excessive_inline_tests`'s decision. This rule
  only arranges files that are *already* extracted; it reads the
  out-of-line `mod` declarations and never the contents of an
  inline `#[cfg(test)] mod tests { ... }` block.
- **Integration tests.** Files under `tests/` are a separate
  compilation target, not unit tests of a subject file, and are out
  of scope (matching `perfectionist::excessive_inline_tests`, which
  also leaves the `tests/` target untouched).

## Configuration

Configure via `dylint.toml` under
`["perfectionist::unit_test_file_layout_mismatch"]`.

```toml
[perfectionist::unit_test_file_layout_mismatch]
# Inactive by default. Enable in `[perfectionist].enable`, then set
# `style` — it is mandatory and has no default. The value below is an
# example, not a default.
style = "aggregated"
# "siblings"   — the subject file declares each test group directly
#                as `#[cfg(test)] mod <group>;`; flag an aggregator
#                funnel (a single `#[cfg(test)] mod tests;` whose file
#                only re-declares the groups).
# "aggregated" — the subject file declares a single
#                `#[cfg(test)] mod tests;` backed by an aggregator
#                file; flag a subject that declares several sibling
#                `#[cfg(test)] mod <group>;` modules directly.

# Name of the aggregator module the `aggregated` style introduces and
# the `siblings` style removes. Defaults to `tests`.
aggregator_module = "tests"

# Minimum number of out-of-line test group files a subject must have
# before the layout is enforced. Defaults to 2 — a single extracted
# file is `excessive_inline_tests`'s target, not a split.
min_files = 2
```

`style` is **mandatory whenever the rule is enabled** and has no
default — *siblings* and *aggregated* are opposite directions on one
axis with no neutral baseline, so the rule ships no `preserve`
variant (see
[Mandatory configuration on opt-in rules](./IMPLEMENTATION_CONVENTIONS.md#mandatory-configuration-on-opt-in-rules)).
A rule that is not enabled never reads its configuration block, so
omitting `style` while the rule is disabled is harmless; only an
*enabled* rule with a missing or invalid `style` is a configuration
error.

## Style: `siblings`

**Avoid:** the tests funnelled through a single aggregator.

```text
test_subject.rs
test_subject/tests.rs            <- mod group_1; mod group_2; mod group_3;
test_subject/tests/group_1.rs
test_subject/tests/group_2.rs
test_subject/tests/group_3.rs
```

```rust
// test_subject.rs
#[cfg(test)]
mod tests;
```

**Prefer:** each group declared directly in the subject file.

```text
test_subject.rs
test_subject/group_1.rs
test_subject/group_2.rs
test_subject/group_3.rs
```

```rust
// test_subject.rs
#[cfg(test)] mod group_1;
#[cfg(test)] mod group_2;
#[cfg(test)] mod group_3;
```

The diagnostic anchors on the subject file's `#[cfg(test)] mod
<aggregator>;` declaration and describes the required moves:
delete the aggregator file, lift each group file up one directory
(`test_subject/tests/group_n.rs` → `test_subject/group_n.rs`), and
replace the single aggregator declaration with one
`#[cfg(test)] mod group_n;` per group.

## Style: `aggregated`

**Avoid:** several sibling test modules declared in the subject
file.

```text
test_subject.rs
test_subject/group_1.rs
test_subject/group_2.rs
test_subject/group_3.rs
```

```rust
// test_subject.rs
#[cfg(test)] mod group_1;
#[cfg(test)] mod group_2;
#[cfg(test)] mod group_3;
```

**Prefer:** a single aggregator behind one test-module line.

```text
test_subject.rs
test_subject/tests.rs            <- mod group_1; mod group_2; mod group_3;
test_subject/tests/group_1.rs
test_subject/tests/group_2.rs
test_subject/tests/group_3.rs
```

```rust
// test_subject.rs
#[cfg(test)]
mod tests;
```

The diagnostic anchors on the first sibling
`#[cfg(test)] mod <group>;` declaration in the subject file and
describes the required moves: create the aggregator file
`test_subject/<aggregator_module>.rs` containing one plain
`mod group_n;` per group, push each group file down one directory
(`test_subject/group_n.rs` →
`test_subject/<aggregator_module>/group_n.rs`), and replace the
sibling declarations with the single
`#[cfg(test)] mod <aggregator_module>;`.

## No machine-applicable autofix

Unlike most rules in this catalogue, the fix here is a filesystem
reorganisation — creating and deleting files and moving them between
directories — which a `rustc`/`cargo fix` span replacement cannot
perform: a span suggestion can only rewrite source *text*, never
`mv` a file or `mkdir` a directory. Rewriting only the `mod`
declarations without moving the backing files would leave the crate
unbuildable, which is worse than no fix. So the rule emits a
**help-only** diagnostic spelling out the moves (as the *Avoid* /
*Prefer* sections do) and never auto-rewrites. State this limitation
in the user-facing `declare_tool_lint!` doc *behaviourally* — that
the fix is a filesystem move the lint cannot apply for you, so it
only emits guidance — without naming the pass internals that cause
it, per
[`declare_tool_lint!` docs describe behaviour, not pass machinery](./IMPLEMENTATION_CONVENTIONS.md#declare_tool_lint-docs-describe-behaviour-not-pass-machinery).
That way the absence of a `MachineApplicable` suggestion reads as
expected, not an oversight.

## What to lint

For every subject module file `<subject>.rs` that backs a module in
the crate:

1. Collect its out-of-line **test** module declarations — items of
   the form `mod <name>;` (no inline body) carrying a test-gate
   `#[cfg(...)]` predicate. Ignore inline `mod <name> { ... }`
   bodies, non-test modules, and items that are not modules.
2. Apply the configured `style`:
   - **`aggregated`**: if the subject declares `>= min_files`
     sibling test modules whose names are *not* the
     `aggregator_module` name (i.e. they are groups, not a single
     aggregator), flag — the subject should funnel them through one
     `#[cfg(test)] mod <aggregator_module>;`.
   - **`siblings`**: if the subject declares a single test module
     whose backing file is an *aggregator* — its body consists
     solely of `>= min_files` plain `mod <group>;` declarations of
     further test submodules and no other substantive items — flag.
     Recognise the aggregator structurally (a test-gated out-of-line
     module whose file only re-declares modules); the
     `aggregator_module` name drives the *suggestion*, not the
     detection, so a differently-named funnel is still caught.
3. Emit the help-only diagnostic described above, anchored on a HIR
   node in the subject file so a local
   `#[expect(perfectionist::unit_test_file_layout_mismatch)]` on the
   subject module resolves.

A subject below the `min_files` threshold (zero or one test file)
is silent under both styles.

## Implementation notes

This rule reads the **written layout of items across module
scopes** — exactly the source-layout shape that
[`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#reaching-every-module-source-layout-rules)
governs. Implement it as a **`LateLintPass`** driven by the shared
`src/module_reparse.rs` helper, **not** an `EarlyLintPass` module
walk: a pre-expansion walk leaves every out-of-line `mod foo;`
`ModKind::Unloaded` and so skips precisely the separate-file
submodules this rule exists to inspect — the bug that shipped twice
already (see that section). Re-parsing in a late pass reaches every
module file while keeping `#[cfg(...)]` gates intact, which this
rule needs because the modules it inspects are all test-gated.

- **The test gate is the wrinkle, not a detail.** A default
  `cargo dylint` build is *not* under `cfg(test)`, so every
  `#[cfg(test)] mod group;` is cfg-disabled and its file backs **no
  HIR node**. The HIR-liveness scoping that
  `module_reparse::for_each_module_file` /
  `live_module_spans` provides therefore *excludes* exactly the
  files this rule cares about. Do not rely on liveness to find the
  test files. Instead, work from the re-parsed AST (which preserves
  cfg-disabled `mod` declarations) and resolve each test
  declaration's backing file by rustc's own module-file algorithm
  (honouring a `#[path = "..."]` attribute, then `<name>.rs`, then
  `<name>/mod.rs`). This crate forbids `mod.rs` for its own source
  (`clippy::mod_module_files`), but a *consumer* project may use
  either form, so resolve both. Confirm the behaviour with a UI
  fixture whose test modules are real separate files, run under the
  same build configuration the consumer gets.
- **Anchor in the subject file.** The subject's
  `#[cfg(test)] mod ...;` declaration lives in the subject file, so
  anchoring the diagnostic on that item's HIR node (via
  `enclosing_hir::find_enclosing_hir_ids` +
  `clippy_utils::diagnostics::span_lint_hir_and_then`) keeps a
  per-module suppression attribute working. The aggregator's inner
  `mod group;` declarations live in a *different* file under a
  disabled cfg, so never anchor there — a span in a cfg-disabled
  file with no HIR node falls back to the crate root and cannot be
  silenced locally.
- **Proc-macro synthesis.** Module declarations emitted by a
  proc-macro are not author layout; guard with the suppression
  helpers from
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations)
  and add a `ui/unit_test_file_layout_mismatch_proc_macro.rs`
  fixture only if the rule proves vulnerable.
- See
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Interaction with sibling lints

This rule is the natural continuation of
`perfectionist::excessive_inline_tests`
(`src/rules/excessive_inline_tests.rs`). That rule decides
*whether* test code is inline or extracted into one file; this rule
decides *how the extracted files are arranged* once there is more
than one. The two never fire on the same item: `excessive_inline_tests`
looks at inline `#[cfg(test)]` items and at most suggests a single
`mod tests;`, while this rule looks only at the *layout of
out-of-line test module declarations* and only once `min_files` of
them exist. A project can sensibly enable both — extract once long,
then keep the split arranged one way.

## Why one rule instead of two

`siblings` and `aggregated` describe the same axis from opposite
ends: each style's "good" layout is the other's "bad" layout. Two
separate rules would have to coordinate so they never both fire on
the same subject. One rule with a `style` knob keeps the policy in
one place — the same shape as `path_qualification_mismatch`
(`unqualified` vs. `qualified`), `serde_wrapper_form_mismatch`
(`transparent` vs. `from_into`), and `import_grouping_mismatch`
(`single_block` vs. `multi_block`).

## Why restrict this?

This is a stylistic preference, not a correctness issue. Both
arrangements compile and run identically; nothing is broken either
way. A project picks one so that a contributor (or an AI assistant)
extending a split test suite knows where a new group file goes and
how to declare it, and so that diffs and directory trees stay
uniform across subjects. Because neither arrangement is universally
better, the rule ships no baseline and is opt-in.

## Difficulty

**Medium.** The detection itself is a small amount of AST shape
matching (count test-gated out-of-line `mod` declarations; for the
`siblings` direction, peek into the aggregator file and check its
body is only `mod` declarations). The two real costs are (1)
reaching the cfg-disabled test files at all under a non-`cfg(test)`
build, which forces manual module-file resolution rather than HIR
liveness, and (2) accepting that there is no machine-applicable
autofix because the fix moves files. Neither is conceptually hard,
but both are easy to get subtly wrong, so lean on UI fixtures that
use genuine separate files.

## Default state

Inactive by default. *siblings* vs. *aggregated* is a per-project
preference with no defensible baseline, so the rule ships nothing
until enabled in `[perfectionist].enable` with a chosen `style`.
