#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints, perfectionist::import_granularity_mismatch,
    perfectionist::import_grouping_mismatch,
    dead_code,
    unused_imports,
    unused_extern_crates,
    reason = "ui fixture"
)]

// The `thiserror_usage` rule is a pre-expansion
// pass: it inspects the syntactic form of `use` statements
// (`use thiserror::...`, `use thiserror::*`, etc.) without
// consulting macro resolution. The fixture exploits that property
// by faking the `thiserror` crate as a local module — every
// `use thiserror::...` line below resolves against this stub, but
// the lint still keys on the path's first segment (`thiserror`) and
// emits exactly as it would against the real crate.
//
// The derive-side cases (`#[derive(thiserror::Error)]`,
// `#[error(...)]` on a thiserror-derived item) need an `Error` derive
// that actually resolves, which this local stub is not, so they live
// in `ui/thiserror_usage_derive.rs` — same trick, but against a
// proc-macro auxiliary rather than a `mod`. What remains here is the
// `use`-side coverage, which covers every alias-collection branch.
mod thiserror {
    pub struct Error;
}

// Bad: bare `use thiserror::Error;` brings the (stubbed) derive
// macro into scope and primes the rule's alias set.
use thiserror::Error;

// Bad: `use thiserror::*` glob form. Even more aggressive — adds
// each recognised path's last segment to the alias set.
use thiserror::*;

// Bad: aliased import. Still flagged — the rule keys on the use
// path's first segment, not on the local name.
use thiserror::Error as RenamedErrorMacro;

// Bad: re-export form. Same first segment, same diagnostic.
pub use thiserror::Error as ReexportedError;

// Bad: nested form.
use thiserror::{Error as _GroupedAlias};

// Bad: crate-rename form `use thiserror as te;`. The first segment
// of the use is still `thiserror`, so the use itself is flagged.
// The rule also records `te` in `crate_aliases` so that a later
// `#[derive(te::Error)]` derive would resolve back to thiserror.
use thiserror as te;

// Bad: nested-`self` crate-rename form. `use thiserror::{self as
// nested_te};` is semantically equivalent to `use thiserror as
// nested_te;`; the rule filters the synthetic `self` segment so
// the inner tree's effective path is `[thiserror]`, falling into
// the same single-segment crate-alias branch.
use thiserror::{self as _nested_te};

// Bad: chain re-export through a crate alias. `te` was recorded as
// a crate alias for `thiserror` by `use thiserror as te;` above, so
// `te::Error` here resolves to `thiserror::Error`. The two-pass
// alias collector expands the first segment through `crate_aliases`
// during the leaf pass, and `use_tree_references_thiserror`
// expands the same way at the use-site check.
use te::Error as _ChainedAlias;

// Bad: top-level braced form with an empty outer prefix. The
// `check_use` recursion descends into the child `thiserror::Error`
// branch and flags the whole `use` item even though the outer tree
// has no first segment.
use {thiserror::Error as _BracedTopLevel, std::io};

fn main() {}
