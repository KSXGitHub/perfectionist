#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    unused_extern_crates,
    reason = "ui fixture"
)]

// The `prefer_derive_more_over_thiserror` rule is a pre-expansion
// pass: it inspects the syntactic form of `use` statements
// (`use thiserror::...`, `use thiserror::*`, etc.) without
// consulting macro resolution. The fixture exploits that property
// by faking the `thiserror` crate as a local module — every
// `use thiserror::...` line below resolves against this stub, but
// the lint still keys on the path's first segment (`thiserror`) and
// emits exactly as it would against the real crate.
//
// The derive-side cases (`#[derive(thiserror::Error)]`,
// `#[error(...)]` on a thiserror-derived item) need a real
// proc-macro `Error` derive — which would require the test fixture
// to depend on the actual `thiserror` crate or a proc-macro
// auxiliary, neither of which fits this ui test's setup. They are
// exercised manually by reading the rule's implementation; the
// `use`-side coverage here covers every alias-collection branch.
mod thiserror {
    pub struct Error;
}

// Bad: bare `use thiserror::Error;` brings the (stubbed) derive
// macro into scope and primes the rule's alias set.
use thiserror::Error;

// Bad: `use thiserror::*` glob form. Even more aggressive — adds
// every configured path's last segment to the alias set.
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

// Bad: top-level braced form with an empty outer prefix. The
// `check_use` recursion descends into the child `thiserror::Error`
// branch and flags the whole `use` item even though the outer tree
// has no first segment.
use {thiserror::Error as _BracedTopLevel, std::io};

fn main() {}
