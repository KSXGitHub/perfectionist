#![feature(rustc_private)]
// Register the `perfectionist` tool namespace so this crate can carry
// its own `#[expect(perfectionist::...)]` attributes (see
// `CONTROLLING_RULES.md`). Gated on `cfg(dylint_lib = "perfectionist")`
// so a plain `cargo build` / `cargo check` ignores it and needs no
// nightly `register_tool` feature.
#![cfg_attr(dylint_lib = "perfectionist", feature(register_tool))]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]
// The unicode-ellipsis rules' own rustdoc names the U+2026 glyph they
// govern, which the crate-wide `scan_code_spans` dogfooding would
// otherwise flag. `unicode_ellipsis_in_docs` scans comments crate-wide
// (in `check_crate`), so its findings only ever resolve to the crate
// root — per-item `#[expect]` is out of scope and unfulfilled — so the
// expectation lives here, the project-wide site `CONTROLLING_RULES.md`
// prescribes. Gated on the dylint cfg so a plain `cargo build`, where
// the lint never runs, sees no unfulfilled expectation.
#![cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::unicode_ellipsis_in_docs,
        reason = "the unicode-ellipsis rules' rustdoc deliberately names the U+2026 glyph"
    )
)]

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
        bare_email
        bare_issue_reference
        bare_url
        derive_ordering
        flat_module_pattern
        import_granularity
        lint_reason_from_comment
        lint_silence_reason
        macro_argument_binding
        macro_trailing_comma
        non_exhaustive_error
        prefer_derive_more_over_thiserror
        prefer_raw_string
        print_macro_split
        single_letter_closure_param
        single_letter_const_generic
        single_letter_const_item
        single_letter_function_param
        single_letter_generic
        single_letter_let_binding
        single_letter_static_item
        unicode_ellipsis_in_comments
        unicode_ellipsis_in_docs
        unicode_ellipsis_in_panic_messages
        unit_test_file_layout

        // `unknown_perfectionist_lints::register_pass` snapshots the registered
        // `perfectionist::*` lint names out of the `LintStore`, so its pass must
        // be installed after every other rule's `register_lint` call. Keep this
        // rule's registrations last, regardless of alphabetical order — a future
        // rule whose name starts with `v`..=`z` would otherwise silently break
        // the snapshot if these calls were left to alphabetical sorting.
        unknown_perfectionist_lints
    }
}
