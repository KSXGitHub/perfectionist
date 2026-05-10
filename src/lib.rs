#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_session;
extern crate rustc_span;

use rustc_lint::LintStore;
use rustc_session::Session;

dylint_linting::dylint_library!();

mod rules;

#[unsafe(no_mangle)]
#[allow(clippy::no_mangle_with_rust_abi)]
pub fn register_lints(session: &Session, lint_store: &mut LintStore) {
    dylint_linting::init_config(session);

    // Phase 1: register every rule's lint declaration first, so the
    // `LintStore` holds the full set of `perfectionist::*` names before any
    // pass tries to read it back.
    rules::unicode_ellipsis_in_comments::register_lint(lint_store);
    rules::unknown_perfectionist_lints::register_lint(lint_store);

    // Phase 2: install the early/late passes. `unknown_perfectionist_lints`
    // snapshots the registered names at construction time, which is why
    // every lint must be registered above before any pass is installed
    // here.
    rules::unicode_ellipsis_in_comments::register_pass(lint_store);
    rules::unknown_perfectionist_lints::register_pass(lint_store);
}
