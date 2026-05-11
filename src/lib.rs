#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_middle;
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
    rules::flat_module_pattern::register_lint(lint_store);
    rules::macro_trailing_comma::register_lint(lint_store);
    rules::unicode_ellipsis_in_comments::register_lint(lint_store);
    rules::unicode_ellipsis_in_panic_messages::register_lint(lint_store);

    // `unknown_perfectionist_lints::register_pass` snapshots the registered
    // `perfectionist::*` lint names out of the `LintStore`, so its pass must
    // be installed after every other rule's `register_lint` call. Keep this
    // rule's registrations last, regardless of alphabetical order — a future
    // rule whose name starts with `v`..=`z` would otherwise silently break
    // the snapshot if these calls were left to alphabetical sorting.
    rules::unknown_perfectionist_lints::register_lint(lint_store);

    rules::flat_module_pattern::register_pass(lint_store);
    rules::macro_trailing_comma::register_pass(lint_store);
    rules::unicode_ellipsis_in_comments::register_pass(lint_store);
    rules::unicode_ellipsis_in_panic_messages::register_pass(lint_store);

    // `unknown_perfectionist_lints::register_pass` snapshots the registered
    // `perfectionist::*` lint names out of the `LintStore`, so its pass must
    // be installed after every other rule's `register_lint` call. Keep this
    // rule's registrations last, regardless of alphabetical order — a future
    // rule whose name starts with `v`..=`z` would otherwise silently break
    // the snapshot if these calls were left to alphabetical sorting.
    rules::unknown_perfectionist_lints::register_pass(lint_store);
}
