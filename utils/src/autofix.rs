//! Drive a codebase fixture through the rounds of autofixes it takes
//! to settle, and hold each round to what the fixture stored for it.
//!
//! A **round** is a perfectionist autofix, then a Clippy autofix,
//! then a rustfmt pass. A codebase is **settled** once neither linter
//! reports anything about it. A **settling path** is one
//! configuration together with the rounds it takes to settle the
//! codebase under it; a codebase fixture carries one path per
//! configuration, all over the same tree, so the configurations share
//! the fixture, the registry dependencies, and the build cache.
//!
//! What a path asserts, round by round:
//!
//! * the exact shape the round left, stored as one diff against the
//!   round before it,
//! * the exact lints still reported after it, so a lint that is meant
//!   to disappear on round *n* cannot survive it,
//! * that the last round leaves nothing reported at all, which is
//!   what settling means.
//!
//! Bracketing the rounds, a path also asserts the codebase behaves as
//! it did before any of them: its own tests pass before the first
//! round and after the last, and its `main` prints the same bytes
//! either side. A rewrite that compiles and answers differently is
//! the failure these catch and the rest cannot.
//!
//! A path over a codebase of shapes the rules deliberately decline is
//! declared as not settling, and then asserts the complement: that
//! the rounds rewrote nothing, which its stored diffs say by being
//! empty, and that the rules went on reporting the shapes rather than
//! falling silent about them.
//!
//! The number of rounds comes from the fixture rather than from
//! driving the codebase to a fixpoint, so a codebase that settles in
//! fewer rounds than declared, or needs more, is a failure rather
//! than something the harness absorbs.

pub mod config;
pub mod diff;
pub mod expectation;
pub mod lints;
pub mod report;
pub mod round;
pub mod snapshot;

pub use config::{Fixture, SettlingPath};
pub use report::{BLESS, Report};
pub use snapshot::Snapshot;

use crate::manifest::fixture_dylint_toml;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Where the codebase fixtures live, relative to this checkout.
const FIXTURES: &str = "tests/fixtures/autofix";

/// Where the codebase fixtures' build caches live, relative to this
/// checkout. One subdirectory per codebase, because the dependency
/// list is what decides what a cache has to hold, and every settling
/// path over one codebase shares that list.
const CACHE: &str = "target/autofix-fixtures";

/// Drive every settling path of the codebase fixture `codebase`, and
/// panic with every failure any of them reported.
///
/// `repository_dir` is this checkout: the fixtures, the build cache,
/// and the plugin the fixture lints under are all found through it.
pub fn check_codebase(repository_dir: &Path, codebase: &str) {
    let fixtures_dir = repository_dir.join(FIXTURES);
    let codebase_dir = fixtures_dir.join(codebase);
    let fixture = Fixture::load(&fixtures_dir, codebase);
    let target_dir = cache_dir(repository_dir, codebase);
    let stored = snapshot::read(&codebase_dir.join("codebase"));
    let context = Context {
        codebase,
        codebase_dir: &codebase_dir,
        repository_dir,
        target_dir: &target_dir,
        cli_package: fixture.cli_package.as_deref(),
    };

    let failures: Vec<String> = fixture
        .paths
        .iter()
        .filter_map(|path| drive(&context, path, &stored))
        .collect();
    assert!(failures.is_empty(), "\n\n{}", failures.join("\n"));
}

/// Everything a settling path needs that is the same for every path
/// over one codebase.
struct Context<'a> {
    codebase: &'a str,
    codebase_dir: &'a Path,
    repository_dir: &'a Path,
    target_dir: &'a Path,
    cli_package: Option<&'a str>,
}

/// Materialise the codebase, drive it along one settling path, and
/// hand back the failure message where the path came out short.
fn drive(context: &Context, path: &SettlingPath, stored: &Snapshot) -> Option<String> {
    let mut report = Report::new(context.codebase.to_owned(), path.name.clone());
    let project = TempDir::new().expect("create a directory for the codebase fixture");
    let dir = project.path();
    snapshot::write(dir, stored);
    write_dylint_toml(dir, context, path);
    report.record(stored.clone());

    let baseline = run_cli(dir, context, &mut report, "before the first round");
    expect_success(round::test(dir, context.target_dir), &mut report);

    let mut previous = stored.clone();
    for number in 1..=path.rounds {
        apply(dir, context.target_dir, &mut report, number);
        let current = snapshot::read(dir);
        report.record(current.clone());
        check_shape(context, path, number, (&previous, &current), &mut report);
        check_lints(dir, context, path, number, &mut report);
        previous = current;
    }

    expect_success(round::test(dir, context.target_dir), &mut report);
    let settled = run_cli(dir, context, &mut report, "after the last round");
    if let (Some(baseline), Some(settled)) = (baseline, settled) {
        assert_same_output(&baseline, &settled, &mut report);
    }
    report.finish()
}

/// Write the fixture's `dylint.toml`: the plugin's location, then the
/// settling path's own configuration appended to it. The path's file
/// holds only the configuration, since where the plugin sits is this
/// checkout's business rather than the fixture's.
fn write_dylint_toml(project_dir: &Path, context: &Context, path: &SettlingPath) {
    let configuration_file = path.directory(context.codebase_dir).join("dylint.toml");
    let configuration = fs::read_to_string(&configuration_file)
        .unwrap_or_else(|error| panic!("read {}: {error}", configuration_file.display()));
    let contents = format!(
        "{}\n{configuration}",
        fixture_dylint_toml(context.repository_dir),
    );
    let file = project_dir.join("dylint.toml");
    fs::write(&file, contents).unwrap_or_else(|error| panic!("write {}: {error}", file.display()));
}

/// Apply one round: perfectionist's autofixes, then Clippy's, then
/// rustfmt. A failure in any of them is recorded and the round still
/// closes, so the dump holds the tree the failure was about.
fn apply(project_dir: &Path, target_dir: &Path, report: &mut Report, number: usize) {
    for outcome in [
        round::dylint_fix(project_dir, target_dir),
        round::clippy_fix(project_dir, target_dir),
        round::rustfmt(project_dir, target_dir),
    ] {
        let reverted = outcome
            .stderr
            .contains("errors present after applying fixes");
        let command = outcome.command.clone();
        expect_success(outcome, report);
        if reverted {
            report.fail(format!(
                "[round {number}] `{command}` rewrote a crate into code that does not \
                 compile, so it threw away every fix in that crate; the rewrite the rule \
                 handed over is unsound",
            ));
        }
    }
}

/// Hold the round to the diff the fixture stored for it.
fn check_shape(
    context: &Context,
    path: &SettlingPath,
    number: usize,
    rounds: (&Snapshot, &Snapshot),
    report: &mut Report,
) {
    let (before, after) = rounds;
    let file = path
        .directory(context.codebase_dir)
        .join(format!("round-{number}.diff"));
    let what = format!("[round {number}] the shape the round left");
    if let Err(failure) = expectation::check(&file, &diff::unified(before, after), &what) {
        report.fail(failure);
    }
}

/// Hold the round to the lints the fixture stored for it, and the
/// last round to reporting none at all.
fn check_lints(
    project_dir: &Path,
    context: &Context,
    path: &SettlingPath,
    number: usize,
    report: &mut Report,
) {
    let mut reported = BTreeSet::new();
    for check in [round::dylint_check, round::clippy_check] {
        let outcome = check(project_dir, context.target_dir);
        reported.extend(lints::reported(&outcome.stdout));
        expect_success(outcome, report);
    }
    let last = number == path.rounds;
    if last && path.settles && !reported.is_empty() {
        report.fail(format!(
            "[round {number}] the codebase is not settled after its last round; \
             still reported:\n{}",
            lints::render(&reported),
        ));
    }
    if last && !path.settles && reported.is_empty() {
        report.fail(format!(
            "[round {number}] the path declares that the codebase does not settle, \
             but nothing is reported after its last round; either the codebase no \
             longer holds a shape the rules decline, or the declaration is stale",
        ));
    }
    let file = path
        .directory(context.codebase_dir)
        .join(format!("round-{number}.lints"));
    let what = format!("[round {number}] the lints still reported");
    if let Err(failure) = expectation::check(&file, &lints::render(&reported), &what) {
        report.fail(failure);
    }
}

/// Run the fixture's `main` and hand back its stdout, or `None` where
/// the fixture declares none or it did not run at all.
fn run_cli(
    project_dir: &Path,
    context: &Context,
    report: &mut Report,
    when: &str,
) -> Option<String> {
    let package = context.cli_package?;
    let outcome = round::run_cli(project_dir, context.target_dir, package);
    let stdout = outcome.stdout.clone();
    let success = outcome.success;
    expect_success_when(outcome, report, when);
    success.then_some(stdout)
}

/// Hold the rounds to leaving the fixture's own behaviour alone.
///
/// A `main` that prints nothing would pass this by comparing nothing,
/// so the empty case is a failure of the fixture rather than a pass.
fn assert_same_output(baseline: &str, settled: &str, report: &mut Report) {
    if baseline.is_empty() {
        report.fail(
            "the fixture's `main` printed nothing, so comparing its output either side \
             of the rounds would assert nothing; have it print what each crate in the \
             codebase contributes"
                .to_owned(),
        );
        return;
    }
    if baseline == settled {
        return;
    }
    report.fail(format!(
        "the fixture's `main` printed different output after the rounds than before \
         them, so a rewrite changed what the codebase does\n\
         --- before ---\n{baseline}--- after ---\n{settled}--- end ---",
    ));
}

fn expect_success(outcome: round::Outcome, report: &mut Report) {
    expect_success_when(outcome, report, "");
}

/// Record a failed invocation, quoting what it printed. `when` names
/// the moment in the path where that is not already obvious from the
/// command.
fn expect_success_when(outcome: round::Outcome, report: &mut Report, when: &str) {
    if outcome.success {
        return;
    }
    let moment = match when.is_empty() {
        true => String::new(),
        false => format!(" {when}"),
    };
    report.fail(format!(
        "`{}`{moment} failed; it printed:\n{}",
        outcome.command, outcome.stderr,
    ));
}

/// The build cache every settling path of `codebase` shares, as
/// [`check_codebase`] computes it.
pub fn cache_dir(repository_dir: &Path, codebase: &str) -> PathBuf {
    repository_dir.join(CACHE).join(codebase)
}

/// Every codebase fixture in this checkout, in ascending order.
///
/// Read off disk rather than listed, so it cannot go stale. The tests
/// name their codebases one by one instead, which is what gives each
/// a failure of its own and lets two run at once; a codebase nobody
/// named is one nothing drives, and is for review to catch.
pub fn codebases(repository_dir: &Path) -> Vec<String> {
    let fixtures_dir = repository_dir.join(FIXTURES);
    let entries = fs::read_dir(&fixtures_dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", fixtures_dir.display()));
    let mut names: Vec<String> = entries
        .map(|entry| entry.expect("read an entry of the codebase fixture directory"))
        .filter(|entry| entry.path().join("fixture.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Fill `codebase`'s build cache, so a later run pays for the
/// standard library, the registry dependencies and the plugin from
/// the cache rather than from cold.
///
/// Everything a round compiles is compiled here: each linter has a
/// wrapper of its own, so each gets a cache lane of its own, and the
/// fixture's own tests and `main` are built by plain rustc, which is
/// a third. A fix pass is not run, so nothing the warmup does depends
/// on which settling path comes first; the first path's configuration
/// is written only because a check needs some configuration to read.
pub fn warm(repository_dir: &Path, codebase: &str) {
    let fixtures_dir = repository_dir.join(FIXTURES);
    let codebase_dir = fixtures_dir.join(codebase);
    let fixture = Fixture::load(&fixtures_dir, codebase);
    let target_dir = cache_dir(repository_dir, codebase);
    let context = Context {
        codebase,
        codebase_dir: &codebase_dir,
        repository_dir,
        target_dir: &target_dir,
        cli_package: fixture.cli_package.as_deref(),
    };
    let project = TempDir::new().expect("create a directory for the codebase fixture");
    let dir = project.path();
    snapshot::write(dir, &snapshot::read(&codebase_dir.join("codebase")));
    let first = fixture.paths.first().expect("a fixture declares a path");
    write_dylint_toml(dir, &context, first);

    let mut report = Report::new(codebase.to_owned(), first.name.clone());
    for check in [round::dylint_check, round::clippy_check] {
        expect_success(check(dir, &target_dir), &mut report);
    }
    expect_success(round::build_tests(dir, &target_dir), &mut report);
    if let Some(failure) = report.finish() {
        panic!("\n{failure}");
    }
}
