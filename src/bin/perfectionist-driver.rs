//! Custom `rustc` driver: forwards everything to upstream rustc, but
//! injects perfectionist's lints via `Callbacks::config`.
//!
//! Cargo invokes us via `RUSTC_WORKSPACE_WRAPPER`, so the actual argv
//! shape is `[driver, /path/to/real/rustc, ...rustc-args]`. We drop
//! the rustc-path argument because *we* are the rustc rustc_driver
//! consumes — we don't shell out to the wrapped binary.

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_session;

// Share the env-var contract with the launcher via `#[path]` to avoid
// pulling the rest of the launcher (clap, ureq, ...) into this binary.
#[path = "../launcher/driver_env.rs"]
mod driver_env;

use std::env;
use std::path::Path;
use std::process::ExitCode;

use driver_env::SYSROOT_ENV;

struct PerfectionistCallbacks;

impl rustc_driver::Callbacks for PerfectionistCallbacks {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        // Chain after any previously-installed registrar so we don't
        // clobber e.g. rustc's built-in tool lints.
        let previous = config.register_lints.take();
        config.register_lints = Some(Box::new(move |sess, lint_store| {
            if let Some(prev) = &previous {
                prev(sess, lint_store);
            }
            perfectionist::register_lints(sess, lint_store);
        }));
    }
}

fn main() -> ExitCode {
    let early_dcx =
        rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
    rustc_driver::init_rustc_env_logger(&early_dcx);

    let mut argv: Vec<String> = env::args().collect();

    // RUSTC_WORKSPACE_WRAPPER calling convention: argv[1] is the path to
    // the real rustc. Detect by file stem so the check works whether
    // cargo passes "rustc", "rustc.exe", or an absolute path.
    let wrapper_mode = argv
        .get(1)
        .map(Path::new)
        .and_then(Path::file_stem)
        .is_some_and(|stem| stem == "rustc");
    if wrapper_mode {
        argv.remove(1);
    }

    // Cargo doesn't always pass `--sysroot`, particularly for `--print`
    // queries that happen during workspace metadata gathering. Inject
    // the launcher-resolved sysroot if missing so the driver always
    // knows where to find std rlibs.
    let has_sysroot = argv
        .iter()
        .any(|a| a == "--sysroot" || a.starts_with("--sysroot="));
    if !has_sysroot && let Ok(sysroot) = env::var(SYSROOT_ENV) {
        argv.push("--sysroot".to_string());
        argv.push(sysroot);
    }

    rustc_driver::catch_with_exit_code(|| {
        rustc_driver::run_compiler(&argv, &mut PerfectionistCallbacks);
    })
}
