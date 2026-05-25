# `import_grouping`

**Source:** project convention. Distinct from
`perfectionist::import_granularity` (`src/rules/import_granularity.rs`),
which decides *merge vs separate*; this rule decides *how use
statements are partitioned into blocks*.

## Statement

A project picks one of two grouping styles and enforces it consistently:

- **`single_group`** — every `use` statement sits in one contiguous
  block. No blank lines between imports.
- **`grouped`** — imports are partitioned into ordered groups
  separated by exactly one blank line. The default group set is:
  1. `std` / `core` / `alloc`.
  2. Internal: `super`, `self`, `crate` (and any prefix of `super::`).
  3. Third-party: every other crate.

The default group order is `["std", "internal", "thirdparty"]`,
configurable. Within each group, the inner ordering is left to
`cargo fmt`.

This rule only governs the *partitioning* of imports. Whether items
within each `use` are merged or split is the job of
`perfectionist::import_granularity`.

## Configuration

```toml
# dylint.toml
[import_grouping]
style = "grouped"   # or "single_group"
order = ["std", "internal", "thirdparty"]
```

Other knobs:

- `import_grouping.std_crates = ["std", "core", "alloc"]` — extend
  with `proc_macro` or `test` if a project routinely imports them.
- `import_grouping.internal_prefixes = ["crate", "super", "self"]` —
  extend with project-specific re-export roots, e.g.
  `"my_workspace_macros"` if it is treated as part of the workspace.
- `import_grouping.cfg_block_handling = "trailing"` — `"trailing"`
  (default) treats a `#[cfg(...)]`-gated import block as a fourth,
  always-last group, matching the convention from both source
  documents in this repository's catalogue. `"merge"` slots cfg-gated
  imports back into their natural group based on the imported path.
- `import_grouping.blank_line_count = 1` — strict equality. Most
  projects want exactly one blank line between groups; bump to `2`
  for the rare style that double-spaces.

## Style: `single_group`

```rust
// Good
use clap::Parser;
use crate::{args::Args, size::Bytes};
use serde::Deserialize;
use std::{io::stdin, time::Duration};

// Bad: a blank line creates a group boundary
use clap::Parser;
use serde::Deserialize;

use std::{io::stdin, time::Duration};
```

The lint flags any blank line that appears *inside* the contiguous
import block at the top of the module body.

## Style: `grouped`

```rust
// Good (default order: std, internal, thirdparty)
use std::{io::stdin, time::Duration};

use crate::{args::Args, size::Bytes};
use super::helpers::compose;

use clap::Parser;
use serde::Deserialize;
```

```rust
// Bad: third-party crate intermixed with std
use clap::Parser;
use std::time::Duration;
use serde::Deserialize;

// Bad: missing blank line between groups
use std::time::Duration;
use crate::args::Args;
use clap::Parser;
```

### Group classification

For each `use` statement, look at the *first segment* of the path:

- Match against `std_crates` → group `std`.
- Match against `internal_prefixes` → group `internal`.
- Otherwise → group `thirdparty`.

Special cases:

- `extern crate foo;` declarations are treated as `thirdparty` for
  classification but kept above all `use` statements (this matches
  rustfmt's behaviour and avoids spurious diagnostics on Rust 2015
  crates).
- A `pub use` is classified by the same rule as a private `use`.
- A `#[cfg(...)]`-gated import is handled per `cfg_block_handling`.

## Implementation notes

- `EarlyLintPass::check_mod`. Walk the module body, identify the
  contiguous run of `use` items at the top (or the first run of
  `use` items if extra items appear), and classify each.
- For `single_group`: emit if any blank line sits between two `use`
  items in the run. Detect blank lines by comparing line numbers via
  `cx.sess.source_map().lookup_line`.
- For `grouped`:
  - Walk the run twice. First pass: assign each `use` to its group.
    Second pass: confirm groups appear in the configured order and
    that exactly `blank_line_count` blank lines separate adjacent
    groups.
  - Emit a single `MachineApplicable` suggestion that re-renders the
    block in the correct shape. Re-rendering needs the original
    text of each `use` (preserved via `snippet_opt`) plus the
    computed group assignments — no semantic changes, only
    re-ordering and blank-line insertion.
- Interaction with `perfectionist::import_granularity`: run
  granularity *first* (or apply both sequentially in a fix pass) so the
  merged output matches the granularity style before grouping decides
  where each `use` line lives.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Interaction with `cargo fmt`

`rustfmt`'s `group_imports` option offers `Preserve`,
`StdExternalCrate`, and `One`. Mapping to this lint:

- `One` ⇔ `style = "single_group"`.
- `StdExternalCrate` ⇔ `style = "grouped"` with
  `order = ["std", "thirdparty", "internal"]` (rustfmt puts `crate`
  last; this rule defaults to internal-second instead because both
  source documents in the catalogue read top-down from `std` to
  external).

Both `group_imports` and `imports_granularity` are unstable rustfmt
options; this lint exists for projects on stable.

## Default state

Active by default. A mismatch with the configured `style` is the
violation; neither style is "wrong" in the abstract, so the rule
is purely about consistency with the project's chosen layout.
