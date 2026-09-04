// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// aux-build:thiserror_stub.rs
#![allow(dead_code, reason = "ui fixture")]

// The derive- and attribute-side halves of `thiserror_usage`, which
// `ui/thiserror_usage.rs` cannot reach: its `thiserror` is a local
// `mod` stub with no derive macro in it, so `#[derive(thiserror::Error)]`
// there would fail to resolve and `#[error(...)]` would be an unknown
// attribute.
//
// This fixture instead builds against `ui/auxiliary/thiserror_stub.rs`,
// a proc-macro crate whose inert `Error` derive declares the same
// helper attributes the real one does. It is renamed to `thiserror` on
// import, both because the rule keys on that name and because the
// dylint driver's `rustc_private` sysroot already ships a crate called
// `thiserror` that would otherwise win the lookup. The rename means
// this line is *not* flagged (the rule reads the original crate name,
// `thiserror_stub`); `extern crate thiserror;` proper is an import-side
// shape, and the import side is covered by `ui/thiserror_usage.rs`.
extern crate thiserror_stub as thiserror;

// Bad: derive named by its full path, plus the `#[error(...)]`
// attribute on the derived item.
#[derive(thiserror::Error)]
#[error("boom")]
struct FullPathDerive;

// Bad: bare `#[derive(Error)]`, resolved through the sibling
// `use thiserror::Error;` the alias scan recorded. (The `use` itself is
// flagged too — its first segment is `thiserror`.)
use thiserror::Error;

#[derive(Error)]
#[error("aliased")]
struct BareDerive;

// Bad: crate-aliased derive path, resolved through `use thiserror as
// te;`.
use thiserror as te;

#[derive(te::Error)]
#[error("crate-aliased")]
struct CrateAliasedDerive;

// Bad: `#[cfg_attr(_, derive(thiserror::Error))]` is unwrapped, and so
// is the sibling `#[cfg_attr(_, error(...))]`.
#[cfg_attr(all(), derive(thiserror::Error))]
#[cfg_attr(all(), error("conditional"))]
struct CfgAttrDerive;

// Bad: `#[error(...)]` on a struct's fields, not just on the item.
#[derive(thiserror::Error)]
struct FieldAttrs {
    #[error("field")]
    inner: u8,
}

// Bad: `#[error(...)]` on an enum's variants and on a variant's
// fields.
#[derive(thiserror::Error)]
enum VariantAttrs {
    #[error("unit variant")]
    Unit,
    Struct {
        #[error("variant field")]
        inner: u8,
    },
}

// Good: a derive list with no thiserror entry is left alone, and the
// `#[error(...)]`-shaped attribute on it is not flagged either — the
// attribute walk only runs on items the derive check classified as
// thiserror-derived.
#[derive(Clone)]
#[cfg_attr(any(), error("never applied"))]
struct NotThiserror;

fn main() {}
