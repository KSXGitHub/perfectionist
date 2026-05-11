//! UI tests for `macro_trailing_comma`'s configuration knobs. The
//! default-config sweep lives in `ui/macro_trailing_comma.rs` and is
//! picked up by `tests/ui.rs`; these tests each point at their own
//! one-fixture directory under `ui-toml/macro_trailing_comma/` and
//! pass a per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! three `#[test]`s in this binary serialise themselves on a shared
//! `Mutex` to avoid clobbering each other under the default
//! parallel test harness.

use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::macro_trailing_comma";

static SERIAL: Mutex<()> = Mutex::new(());

fn dylint_toml_for(body: &str) -> String {
    format!("[\"{LINT_NAME}\"]\n{body}")
}

fn run(src_base: &str, dylint_toml_body: &str) {
    // A poisoned mutex from a previous panic doesn't make this lock
    // unsafe — recover the inner guard and proceed.
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml_for(dylint_toml_body))
        .run();
}

#[test]
fn extra_name_based_enables_a_user_named_macro() {
    run(
        "ui-toml/macro_trailing_comma/extra_name_based",
        "extra_name_based = [\"my_macro\"]\n",
    );
}

#[test]
fn ignore_suppresses_a_built_in_curated_macro() {
    run(
        "ui-toml/macro_trailing_comma/ignore",
        "ignore = [\"vec\"]\n",
    );
}

#[test]
fn ignore_wins_over_extra_name_based_for_the_same_macro() {
    run(
        "ui-toml/macro_trailing_comma/ignore_overrides_extra",
        "extra_name_based = [\"my_macro\"]\nignore = [\"my_macro\"]\n",
    );
}
