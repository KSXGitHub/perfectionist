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
mod attr_tokens;
mod cargo_target;
mod comment_walk;
mod common;
mod derive_list;
mod enclosing_hir;
mod format_template;
mod literal_scan;
mod macro_path;
mod macro_template;
mod markdown;
mod measured_fn;
mod module_reparse;
mod rule_index;
mod rules;
mod test_code;
mod url_scan;

#[unsafe(no_mangle)]
#[expect(
    clippy::no_mangle_with_rust_abi,
    reason = "dylint's plugin entry point requires the Rust ABI"
)]
pub fn register_lints(session: &Session, lint_store: &mut LintStore) {
    dylint_linting::init_config(session);
    common::init_global_config();
    rule_index::register_all(lint_store);
}
