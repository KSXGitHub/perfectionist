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

mod unicode_ellipsis_in_comments;
mod unknown_perfectionist_lints;

#[unsafe(no_mangle)]
#[allow(clippy::no_mangle_with_rust_abi)]
pub fn register_lints(sess: &Session, lint_store: &mut LintStore) {
    dylint_linting::init_config(sess);

    // Each rule module registers its own lints (and any pass that does not
    // depend on the full lint set).
    unicode_ellipsis_in_comments::register(lint_store);
    unknown_perfectionist_lints::register_lint(lint_store);

    // `unknown_perfectionist_lints` validates `#[allow(perfectionist::...)]`
    // attributes against the registered lint set. Its pass reads that set
    // out of `lint_store`, so its installation must come after every
    // module above has registered its lints. Keep this call last.
    unknown_perfectionist_lints::register_pass(lint_store);
}
