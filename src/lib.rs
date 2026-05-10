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

/// Names of every lint this plugin registers, stripped of the
/// `perfectionist::` tool prefix. Add new lints here as they are
/// implemented; `unknown_perfectionist_lints` consults this list
/// to validate `#[allow(perfectionist::...)]` attributes.
const REGISTERED_LINT_NAMES: &[&str] = &[
    "unicode_ellipsis_in_comments",
    "unknown_perfectionist_lints",
];

#[unsafe(no_mangle)]
#[allow(clippy::no_mangle_with_rust_abi)]
pub fn register_lints(sess: &Session, lint_store: &mut LintStore) {
    dylint_linting::init_config(sess);
    unicode_ellipsis_in_comments::register(lint_store);
    unknown_perfectionist_lints::register(lint_store, REGISTERED_LINT_NAMES);
}
