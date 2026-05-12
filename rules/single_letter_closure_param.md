# `perfectionist::single_letter_closure_param`

**Default level:** `warn`  
**Source:** [`src/rules/single_letter_closure_param.rs`](../src/rules/single_letter_closure_param.rs)

> closure parameter has a single-letter name

### What it does
Flags closure parameters whose identifier is one ASCII
letter, unless the closure is a trivial single-expression
callback. Two shapes qualify as trivial:
- the closure is the immediate argument of a call whose
  callee name is in the comparison / fold allowlist
  (`sort_by`, `sort_by_key`, `min_by`, `max_by`,
  `binary_search_by`, `cmp_by`, `partial_cmp_by`,
  `fold`, `try_fold`, …);
- the body is a trivial wrapper around the parameter —
  a field access (`|x| x.field`), a method call
  (`|x| x.foo()`), a one-argument call where the
  parameter is the sole argument (`|x| vec![x]`), or a
  reference (`|x| &x`). Surrounding `*` / `&` operators
  around the parameter inside any of these shapes are
  peeled before the match, so `|s| (*s).foo()` qualifies.

### Why restrict this?
This is a stylistic preference, not a correctness issue.
A multi-line closure body whose parameter is a single
letter forces the reader to scroll back to the closure
header for context on every reference. The trivial-
callback exception covers `sort_by(|a, b| ...)` and
`.map(|x| x.field)` shapes that are short enough that the
parameter's role is unambiguous from the call site.

### Example
```rust,ignore
.map(|t| {
    let columns = build_columns(t);
    format_row(&columns)
})
```
Use instead:
```rust,ignore
.map(|tree_row| {
    let columns = build_columns(tree_row);
    format_row(&columns)
})
```

## Configuration

Configure via `dylint.toml` under `["perfectionist::single_letter_closure_param"]`. Every field is optional; the per-field prose below states the default.

### `comparison_methods` — `[string]` (optional)

Method / function names whose closure argument may carry
single-letter parameters when the body is a single
expression. Extend this list to add project-specific DSL
helpers (`when`, `iter_by`, …).
