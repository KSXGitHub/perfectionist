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

mod ascii_letter;
mod comment_walk;
mod common;
mod enclosing_hir;
mod literal_scan;
mod macro_path;
mod markdown;
mod rules;
mod url_scan;

#[unsafe(no_mangle)]
#[allow(
    clippy::no_mangle_with_rust_abi,
    reason = "dylint's plugin entry point requires the Rust ABI"
)]
pub fn register_lints(session: &Session, lint_store: &mut LintStore) {
    dylint_linting::init_config(session);
    common::init_global_config();

    macro_rules! register {
        ($( $rule_name:ident )+) => {
            $( rules::$rule_name::register_lint(lint_store); )+
            $( rules::$rule_name::register_pass(lint_store); )+
        };
    }

    register! {
        arc_rc_clone
        bare_email
        bare_issue_reference
        bare_url
        derive_ordering
        flat_module_pattern
        lint_silence_reason
        macro_argument_binding
        macro_trailing_comma
        non_exhaustive_error
        prefer_raw_string
        single_letter_closure_param
        single_letter_function_param
        single_letter_generic
        single_letter_let_binding
        unicode_ellipsis_in_comments
        unicode_ellipsis_in_panic_messages

        // `unknown_perfectionist_lints::register_pass` snapshots the registered
        // `perfectionist::*` lint names out of the `LintStore`, so its pass must
        // be installed after every other rule's `register_lint` call. Keep this
        // rule's registrations last, regardless of alphabetical order — a future
        // rule whose name starts with `v`..=`z` would otherwise silently break
        // the snapshot if these calls were left to alphabetical sorting.
        unknown_perfectionist_lints
    }
}
