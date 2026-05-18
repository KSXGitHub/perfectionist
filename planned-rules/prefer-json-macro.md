# `prefer_json_macro`

**Source:** project convention.

## Statement

Test code routinely fabricates small JSON documents — fixtures
written to disk, payloads handed to subprocesses, request bodies
posted to a test server. The natural Rust idiom is
`serde_json::json!`:

```rust
let manifest = serde_json::json!({
    "name": "test",
    "version": "0.0.0",
    "scripts": {
        "touch-marker": format!("touch {}", marker_path.display()),
    },
})
.to_string();
fs::write(&manifest_path, manifest).expect("write package.json");
```

Hand-constructing the same payload with a raw or formatted string
literal looks like:

```rust
let manifest = format!(
    r#"{{
        "name": "test",
        "version": "0.0.0",
        "scripts": {{ "touch-marker": "touch {marker}" }}
    }}"#,
    marker = marker_path.display(),
);
fs::write(&manifest_path, manifest).expect("write package.json");
```

This rule rewrites the second form into the first whenever it
appears in test code. It is silent in production code, where the
overhead of building a `serde_json::Value` and re-serialising it
is sometimes the deciding factor and reviewers are best placed to
choose case by case.

## Why restrict this?

This is a stylistic preference, not a correctness issue. The
string-literal and `format!` forms produce valid JSON when
written correctly; the issue is that "written correctly" is a
property the author has to maintain by hand:

- Escapes inside interpolated runtime values silently corrupt the
  output. `{marker}` for a path containing `"` produces invalid
  JSON; `json!` routes the value through `serde_json::Value` and
  escapes it per the JSON grammar.
- Structural typos (missing comma, stray `}}`, mismatched
  bracket) pass `cargo build` and surface only when the
  downstream consumer parses the result. `json!` rejects an
  invalid document at the call site.
- The doubled-brace `{{` / `}}` noise in `format!` templates is
  visual clutter that the macro form removes.
- The macro syntax matches the document's structure, so editing
  one field is a structural edit a `cargo fmt` pass handles
  rather than a whitespace-and-quoting rebalance.

These costs are usually worth paying in production code where
the construction is on a hot path; in test code, where the JSON
construction runs once per case and clarity dominates, `json!`
wins.

## Activation

The rule is silent in workspaces that do not use `serde_json` at
all. It activates when **any** of the following holds:

1. The workspace's root `Cargo.toml` declares `serde_json` in
   `[workspace.dependencies]` (or any workspace-level dependency
   table).
2. The local crate's own `Cargo.toml` declares `serde_json` in
   `[dependencies]` or `[dev-dependencies]`.
3. *Any* other crate in the same workspace declares `serde_json`
   in `[dependencies]` or `[dev-dependencies]`. A multi-crate
   workspace where one crate already uses `serde_json` for
   fixtures counts; the lint expects the same convention across
   the workspace.

Activation is computed once per `dylint` run, before per-file
analysis begins, and cached as a single boolean on the lint
pass. When activation fails the rule emits no diagnostics — the
autofix would otherwise suggest a dependency the project hasn't
opted into, which the user cannot apply without a separate
`cargo add`. No "rule was skipped" warning is produced; silence
is the correct behaviour for a project that has legitimately
opted out by not depending on `serde_json`.

## What to lint

Fire only on expressions that satisfy **all** of:

1. **The expression is in a test context.** Test context is any
   of:
   - the enclosing `fn` carries `#[test]`, `#[tokio::test]`,
     `#[async_std::test]`, or any other attribute matching
     `test_attributes` (configurable);
   - the enclosing module or any ancestor module carries
     `#[cfg(test)]`;
   - the source file is at or under a directory named in
     `test_directories` at the crate root (default `tests/`).
     `benches/` is **not** included by default — bench code is
     performance-sensitive, the same rationale that excludes
     production code — but can be added via configuration.
2. **The expression's runtime value is a JSON document.** Two
   detection modes:
   - *Literal mode.* The expression is a string literal whose
     decoded value parses as JSON via `serde_json::from_str`.
   - *Format mode.* The expression is a `format!` invocation
     whose template, after replacing every `{...}` placeholder
     with the literal JSON value `null` and unescaping `{{` /
     `}}` to `{` / `}`, parses as JSON via
     `serde_json::from_str`. `format_args!`, `write!`,
     `writeln!`, `print!`-family macros are out of scope — they
     are typically used for streaming output, not JSON
     fabrication.
3. **The document is structurally interesting.** Defined
   inductively over the parsed JSON value:
   - a JSON object is structurally interesting iff it has at
     least one key;
   - a JSON array is structurally interesting iff at least one
     of its elements is structurally interesting;
   - every other JSON value (string, number, boolean, null) is
     not structurally interesting.

   The rule fires only when the top-level document is
   structurally interesting. Equivalently: somewhere in the
   value tree there is a JSON object with at least one key.
   This exempts scalar documents (`"hello"`, `42`, `true`,
   `null`), empty containers (`{}`, `[]`), flat primitive arrays
   (`[0, 1, 2, 3]`, `["a", "b"]`), and arbitrarily nested arrays
   that ultimately bottom out in primitives or empty objects
   (`[{}]`, `[[[]]]`). The motivating cases — objects with
   named fields and arrays of such objects — all carry at least
   one non-empty object somewhere in the tree.

For every triggering expression, emit a diagnostic suggesting
`serde_json::json!({ ... }).to_string()` and supply an autofix
where possible (see [Autofix](#autofix)).

## Examples

### Literal mode

```rust
#[test]
fn writes_manifest() {
    // Bad: structural literal, easy to typo
    let manifest = r#"{
        "name": "test",
        "version": "0.0.0",
        "scripts": { "touch-marker": "touch /tmp/marker" }
    }"#;
    fs::write(&manifest_path, manifest).expect("write");
}

// Good
#[test]
fn writes_manifest() {
    let manifest = serde_json::json!({
        "name": "test",
        "version": "0.0.0",
        "scripts": { "touch-marker": "touch /tmp/marker" },
    })
    .to_string();
    fs::write(&manifest_path, manifest).expect("write");
}
```

### Format mode

```rust
#[test]
fn writes_manifest_with_marker() {
    // Bad: doubled braces, runtime value could contain `"` and
    // corrupt the output
    let manifest = format!(
        r#"{{
            "name": "test",
            "version": "0.0.0",
            "scripts": {{ "echo-args": "sh -c 'printf %s \"$1\" > {marker}' --" }}
        }}"#,
        marker = marker_path.display(),
    );
    fs::write(&manifest_path, manifest).expect("write");
}

// Good
#[test]
fn writes_manifest_with_marker() {
    let manifest = serde_json::json!({
        "name": "test",
        "version": "0.0.0",
        "scripts": {
            "echo-args": format!(
                "sh -c 'printf %s \"$1\" > {}' --",
                marker_path.display(),
            ),
        },
    })
    .to_string();
    fs::write(&manifest_path, manifest).expect("write");
}
```

### Skipped contexts

```rust
// Skipped: not in a test context (no `#[cfg(test)]`, not under
// `tests/`, no `#[test]` attribute on the enclosing fn).
fn render_payload() -> String {
    r#"{"name":"hot path","fast":true}"#.to_string()
}

// Skipped: scalar document
#[test]
fn scalar() {
    let _ = r#""hello""#;
}

// Skipped: structurally uninteresting — no non-empty object
// anywhere in the tree.
#[test]
fn flat_primitive_array() {
    let _ = "[0, 1, 2, 3]";
    let _ = "{}";
    let _ = "[[]]";
}

// Skipped: not valid JSON
#[test]
fn not_json() {
    let _ = "name=test, version=0.0.0";
}
```

## Configuration

```toml
[prefer_json_macro]
# Disable the rule entirely. Useful for workspaces that already
# have a strong reason to hand-write JSON in test code (e.g.,
# tests that verify byte-exact serializer output).
enabled = true

# Attribute paths whose presence on the enclosing function counts
# as "this is a test". Entries without `::` match against the
# attribute's last path segment; entries containing `::` match
# against the full path.
test_attributes = [
  "test",
  "tokio::test",
  "async_std::test",
  "actix_web::test",
  "actix_rt::test",
  "rstest::rstest",
  "test_case::test_case",
]

# Directory names directly under the crate root whose contents
# are treated as test code. Files under any subdirectory of these
# count too.
test_directories = ["tests"]

# Override the import path the autofix uses for the `json!` macro.
# Set this if the project re-exports `json!` from a wrapper
# module (`crate::test_utils::json!`).
json_macro_path = "serde_json::json"
```

## Implementation notes

- **Activation probe.** At pass construction, locate the
  workspace root by walking parents of
  `tcx.sess.opts.working_dir` until a `Cargo.toml` containing a
  `[workspace]` table is found, or the crate's own `Cargo.toml`
  if no workspace exists. Parse it with the `toml` crate and
  inspect every `members = [...]` entry's `Cargo.toml`,
  expanding glob entries (`"crates/*"`) the same way cargo
  does. Search the four relevant tables —
  `workspace.dependencies`, `workspace.dev-dependencies`,
  `dependencies`, `dev-dependencies` — for an entry named
  `serde_json`. Stop at the first match. Cache the boolean on
  the pass struct; subsequent `check_expr` invocations
  short-circuit on `false`.

  This is the only rule in the catalogue that performs
  filesystem I/O outside the source tree. Keep the probe small
  and defensive: a malformed `Cargo.toml` should disable the
  lint, not panic the compiler.

- **Test-context detection.** `LateLintPass::check_expr`. From
  the expression's `HirId`, walk parents via `tcx.hir_parents`
  until an item is reached:
  - If the item is a `fn` with one of `test_attributes` applied,
    fire.
  - If any ancestor module carries `#[cfg(test)]` (detected via
    `attr.has_name(sym::cfg)` and a meta-item walk for
    `cfg(test)` / `cfg(any(..., test, ...))` /
    `cfg(all(..., test))`), fire.
  - If the source file's path (from
    `tcx.sess.source_map()`) is at or under one of the
    configured `test_directories` at the crate root, fire.

  Cache per-`HirId` to keep repeated `check_expr` calls on
  expressions inside the same function cheap.

- **JSON detection — literal mode.** For
  `ExprKind::Lit(LitKind::Str(..))`, run `serde_json::from_str`
  on the decoded value. If parsing succeeds, walk the resulting
  `serde_json::Value` to check the structural-interest
  predicate: an `Object` with at least one key is interesting;
  an `Array` is interesting iff at least one of its elements is
  interesting; everything else is uninteresting. Fire only when
  the top-level value is interesting.

- **JSON detection — format mode.** For a macro invocation that
  resolves to `format!`, locate the template literal (first
  positional argument) via
  `clippy_utils::macros::FormatArgsExpn`. Walk the template
  with the catalogue's shared format-template combinators (see
  [`derive-more-inlined-args`](./derive-more-inlined-args.md)
  and [`format-macro-wrap`](./format-macro-wrap.md)) and emit a
  synthetic string where each `{...}` placeholder is replaced
  by `null` and each `{{` / `}}` is unescaped to `{` / `}`. Run
  `serde_json::from_str` on the synthetic result; if it parses
  and the resulting `Value` is structurally interesting (same
  predicate as literal mode), fire.

  The placeholder-as-`null` substitution is a syntactic
  approximation: it assumes the placeholder slot will hold *a*
  JSON value at runtime. False positives are possible — a
  placeholder could be e.g. a comma-separated array tail
  (`{items}` in `[{items}]`) where the macro form is awkward —
  and the autofix's `Applicability::MaybeIncorrect` reflects
  that.

- **Parser style.** The format-template parser already exists in
  the catalogue; reuse its `take_*` combinators rather than
  re-parsing the template. See
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md).

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Autofix

Synthesise the `serde_json::json!({ ... }).to_string()` form
from the parsed JSON. Each JSON string becomes a Rust string
literal; each format-template placeholder becomes the
corresponding Rust expression from the `format!` argument list.
Placeholders that occupy a string position in the JSON become
`format!("...{}...", expr)` calls (or the placeholder expression
directly when the surrounding string is empty), preserving the
escape semantics that `json!` then handles.

`Applicability::MaybeIncorrect`. The suggestion compiles and
produces the same JSON only when the placeholder expressions
serialise to plain strings (a `PathBuf`'s `Display` output on
Unix matches its JSON serialisation, but `Debug` output does
not, so the rewrite is conservative). When no `use
serde_json::json;` is in scope the suggestion includes the
import; the path is configurable via `json_macro_path`.

### Difficulty

**Hard.** Two layers of analysis stack:

- The activation probe requires reading and parsing
  `Cargo.toml` files from disk before pass setup completes —
  the catalogue's only rule that performs filesystem I/O
  outside the source tree.
- The format-mode JSON detection requires parsing the format
  template *and* speculatively parsing the result as JSON. The
  combinators exist; chaining them is straightforward, but the
  edge cases (placeholder inside a string literal vs. outside,
  placeholder spanning a structural position) require care.

Literal-mode detection is straightforward
(`serde_json::from_str` on the decoded value); the format-mode
detection is where the work concentrates. The autofix is
best-effort and `MaybeIncorrect` regardless.

## Severity

Warn.

## Interaction with sibling rules

- [`prefer-text-block`](./prefer-text-block.md) — both rules
  look at multi-line string literals, but operate on disjoint
  classes. `prefer_text_block` rewrites the literal's *shape*
  (newlines split across source lines); this rule rewrites the
  literal's *construction* (string → `json!` macro). When a
  literal triggers both, this rule's suggestion supersedes
  `prefer_text_block`'s — the `json!` macro produces structured
  data, not a multi-line string, so the text-block reshape
  becomes moot. Run this rule first; if the user accepts the
  suggestion, the text-block lint no longer applies.
- [`format-macro-wrap`](./format-macro-wrap.md) — when a
  `format!` invocation triggers both rules, prefer this one for
  the same reason: the `json!` rewrite eliminates the
  `format!` invocation entirely, making the wrap question moot.
