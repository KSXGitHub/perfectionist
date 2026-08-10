// `include!`d by the sibling `fixture.rs` — this is not an
// `aux-build` crate. It lives under `auxiliary/` only because
// compiletest skips that directory when collecting tests, and the
// fixture needs a second source file that is not itself a fixture.

silence!("rationale supplied from another file");
