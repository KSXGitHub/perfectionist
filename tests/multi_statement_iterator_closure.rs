use std::sync::Mutex;

static SERIAL: Mutex<()> = Mutex::new(());

fn run_fixtures(directory: &str, configuration: &str) {
    let _guard = SERIAL.lock().unwrap();
    let fixtures = _utils::copy_fixtures_with_directives(env!("CARGO_MANIFEST_DIR"), directory);
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(configuration)
        .run();
}

#[test]
fn enabled_with_default_threshold() {
    run_fixtures(
        "ui-toml/multi_statement_iterator_closure/enabled",
        "[perfectionist]\nenable = [\"multi_statement_iterator_closure\"]\n",
    );
}

#[test]
fn threshold_allows_two_statements() {
    run_fixtures(
        "ui-toml/multi_statement_iterator_closure/threshold",
        "[perfectionist]\nenable = [\"multi_statement_iterator_closure\"]\n\
         [\"perfectionist::multi_statement_iterator_closure\"]\nmax_statements = 2\n",
    );
}

#[test]
fn zero_threshold_counts_single_expression_and_allows_empty_body() {
    run_fixtures(
        "ui-toml/multi_statement_iterator_closure/zero",
        "[perfectionist]\nenable = [\"multi_statement_iterator_closure\"]\n\
         [\"perfectionist::multi_statement_iterator_closure\"]\nmax_statements = 0\n",
    );
}
