//! Tests for `multi_statement_iterator_closure`.
//!
//! The rule is inactive by default, so the fixture at
//! `ui/multi_statement_iterator_closure.rs` holds an over-threshold
//! callback against an empty `.stderr`, which is what proves the rule
//! stays off until it is enabled. Every fixture here therefore travels
//! with a `dylint.toml` that enables it: `enabled` pins the default
//! threshold, `threshold` pins a raised one, `zero` pins where a
//! statement starts being counted, and `proc_macro` pins which
//! callbacks a macro's involvement exempts.

/// `[perfectionist]` turns the rule on, since it ships inactive. The
/// rule's own table is omitted, so `max_statements` is its default.
const ENABLED: &str = "[perfectionist]\nenable = [\"multi_statement_iterator_closure\"]\n";

fn run_ui(src_base: &str, dylint_toml: &str) {
    _utils::ConfiguredUiTest::builder()
        .library_name(env!("CARGO_PKG_NAME"))
        .manifest_dir(env!("CARGO_MANIFEST_DIR"))
        .src_base(src_base)
        .dylint_toml(dylint_toml)
        .run();
}

/// No `max_statements` override: the fixture is measured against the
/// rule's own default of 1.
#[test]
fn enabled_with_default_threshold() {
    run_ui("ui-toml/multi_statement_iterator_closure/enabled", ENABLED);
}

/// A raised threshold, which leaves a two-statement callback alone.
#[test]
fn threshold_allows_two_statements() {
    run_ui(
        "ui-toml/multi_statement_iterator_closure/threshold",
        &format!(
            "{ENABLED}[\"perfectionist::multi_statement_iterator_closure\"]\n\
             max_statements = 2\n",
        ),
    );
}

/// A threshold of zero, where a single expression is already one
/// statement and only an empty body is left alone.
#[test]
fn zero_threshold_counts_single_expression_and_allows_empty_body() {
    run_ui(
        "ui-toml/multi_statement_iterator_closure/zero",
        &format!(
            "{ENABLED}[\"perfectionist::multi_statement_iterator_closure\"]\n\
             max_statements = 0\n",
        ),
    );
}

/// A callback a derive synthesised is left alone even where its every
/// token wears a user span. One the author wrote is reported even where
/// an attribute macro rebuilt the signature around it.
#[test]
fn a_rebuilt_signature_does_not_hide_a_handwritten_callback() {
    run_ui(
        "ui-toml/multi_statement_iterator_closure/proc_macro",
        ENABLED,
    );
}
