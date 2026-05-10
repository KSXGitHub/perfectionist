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
pub fn register_lints(sess: &Session, lint_store: &mut LintStore) {
    dylint_linting::init_config(sess);
    rules::flat_module_pattern::register(lint_store);
    rules::unicode_ellipsis_in_comments::register(lint_store);
    rules::unknown_perfectionist_lints::register_lint(lint_store);
    rules::unknown_perfectionist_lints::register_pass(lint_store);
}
