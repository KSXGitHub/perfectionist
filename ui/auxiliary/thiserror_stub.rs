// force-host
// no-prefer-dynamic
//
// A miniature stand-in for `thiserror`. The `Error` derive is inert —
// it emits no tokens, leaving the annotated struct / enum untouched —
// and declares the helper-attribute set (`error`, `source`, `from`,
// `backtrace`) the real derive does, so a fixture can write
// `#[error("...")]` on an item, its variants, and its fields and still
// compile.
//
// `thiserror_usage` is a pre-expansion pass keying on the derive's
// syntactic path, so an inert stand-in exercises the derive- and
// attribute-side emitters exactly as the real crate would.
//
// The crate is deliberately *not* named `thiserror`: the dylint driver
// runs with the `rustc_private` sysroot on the search path, and that
// sysroot ships its own `thiserror`, which wins the name lookup and
// makes the fixture fail to build. The fixture renames it on import
// instead (`extern crate thiserror_stub as thiserror;`).

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::TokenStream;

#[proc_macro_derive(Error, attributes(error, source, from, backtrace))]
pub fn error(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
