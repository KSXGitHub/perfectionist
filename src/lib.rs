#![feature(rustc_private)]
#![cfg_attr(dylint_lib = "perfectionist", feature(register_tool))]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_parse;
extern crate rustc_session;
extern crate rustc_span;

use rustc_lint::LintStore;
use rustc_session::Session;

dylint_linting::dylint_library!();

mod abs_path;
mod ascii_letter;
mod comment_walk;
mod common;
mod enclosing_hir;
mod format_template;
mod literal_scan;
mod macro_path;
mod macro_template;
mod markdown;
mod module_reparse;
mod rules;
mod url_scan;

#[unsafe(no_mangle)]
#[expect(
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
        allow_attributes
        allow_attributes_without_reason
        avoidable_string_escapes
        bare_email
        bare_identifier_reference
        bare_issue_reference
        bare_url
        clap_help_markdown
        excessive_inline_tests
        exhaustive_error_enums
        import_granularity_mismatch
        import_grouping_mismatch
        impure_macro_arguments
        lint_attribute_trailing_comment
        macro_trailing_comma
        named_prelude_imports
        needless_borrowed_parameters
        overly_long_print_macro
        redundant_derive_more_forward_template
        single_letter_closure_param
        single_letter_const_generic
        single_letter_const_item
        single_letter_function_param
        single_letter_generic
        single_letter_let_binding
        single_letter_static_item
        thiserror_usage
        uncombined_self_import
        unicode_ellipsis_in_comments
        unicode_ellipsis_in_docs
        unicode_ellipsis_in_panic_messages
        unordered_derives
        unpinned_repo_ref
        wildcard_imports

        // `unknown_perfectionist_lints::register_pass` snapshots the registered
        // `perfectionist::*` lint names out of the `LintStore`, so its pass must
        // be installed after every other rule's `register_lint` call. Keep this
        // rule's registrations last, regardless of alphabetical order — a future
        // rule whose name starts with `v`..=`z` would otherwise silently break
        // the snapshot if these calls were left to alphabetical sorting.
        unknown_perfectionist_lints
    }
}
