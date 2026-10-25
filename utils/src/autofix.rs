//! Drive a fixture's project through the rounds of autofixes it takes
//! to settle, and hold each round to what the fixture stored for it.
//!
//! A **round** is a perfectionist autofix, then a Clippy autofix,
//! then a rustfmt pass. A project is **settled** once neither linter
//! reports anything about it. A **run** is one configuration together
//! with the rounds it takes to settle the project under it; a fixture
//! carries one run per configuration, all over the same project, so
//! the configurations share the tree, the registry dependencies, and
//! the build cache.
//!
//! What a run asserts, round by round:
//!
//! * the exact shape the round left, stored as one diff against the
//!   round before it,
//! * the exact diagnostics it was handed, normalised as a `.stderr`
//!   under `ui/` is, which is what its stored diff is the answer to:
//!   round *n*'s two files are what round *n* saw and what it did
//!   about it.
//!
//! Once the last round has run, the project has to report nothing at
//! all, which is what settling means. That one is an assertion rather
//! than a stored file, since a blessing run re-records every stored
//! expectation and would absorb a regression that left findings
//! behind.
//!
//! Once the last round has run, a run also asserts the project still
//! behaves: its own tests pass, and its `main` prints the bytes the
//! fixture stored for it. A rewrite that compiles and answers
//! differently is the failure these catch and the rest cannot. Both
//! are asserted only there, that being the only tree whose behaviour
//! is in question, and compiling the project's test targets a second
//! time would cost more than running them over the tree the rounds
//! started from is worth.
//!
//! A run over a project of shapes the rules deliberately decline is
//! declared as not settling, and then asserts the complement: that
//! the rounds rewrote nothing, which its stored diffs say by being
//! empty, and that the rules went on reporting the shapes rather than
//! falling silent about them.
//!
//! The number of rounds comes from the fixture rather than from
//! driving the project to a fixpoint, so a project that settles in
//! fewer rounds than declared, or needs more, is a failure rather
//! than something the harness absorbs.

pub mod config;
pub mod diagnostics;
pub mod diff;
pub mod expectation;
pub mod report;
pub mod round;
pub mod snapshot;

pub use config::{Fixture, Run};
pub use report::{BLESS, Report};
pub use snapshot::Snapshot;

use crate::manifest::fixture_dylint_toml;
use diagnostics::Reported;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Where the fixtures live, relative to this checkout.
const FIXTURES: &str = "tests/fixtures/autofix";

/// What a fixture stores its `main`'s output under, relative to the
/// fixture. One file per fixture rather than per run: every run
/// rewrites the same project, and arriving at the same behaviour
/// whatever it rewrote is the claim.
const OUTPUT: &str = "cli.stdout";

/// Where the fixtures' build caches live, relative to this checkout.
/// One subdirectory per fixture, because the dependency list is what
/// decides what a cache has to hold, and every run over one fixture
/// shares that list.
///
/// What the cache carries across runs is the compiled standard
/// library, the registry dependencies and the plugin, which is where
/// the cost is. It deliberately does not carry the fixture's own
/// members: cargo decides a path dependency's freshness by comparing
/// mtimes and never by reading its contents, so a project
/// materialised with the stored tree's own mtimes would be taken for
/// fresh against artefacts built from a *later* round of an earlier
/// run over the same project, and that run's rewrites would stand in
/// for this one's round zero. Materialising at the current time is
/// what keeps each run's first round honest, and the members are
/// small enough that recompiling them is noise beside the fix passes.
const CACHE: &str = "target/autofix-fixtures";

/// Drive every run of the fixture `name`, and panic with every
/// failure any of them reported.
///
/// `repository_dir` is this checkout: the fixtures, the build cache,
/// and the plugin the project lints under are all found through it.
pub fn check_fixture(repository_dir: &Path, name: &str) {
    let fixtures_dir = repository_dir.join(FIXTURES);
    let fixture_dir = fixtures_dir.join(name);
    let fixture = Fixture::load(&fixtures_dir, name);
    let target_dir = cache_dir(repository_dir, fixture.cache(name));
    let stored = snapshot::read(&fixture_dir.join("project"));
    let context = Context {
        name,
        fixture_dir: &fixture_dir,
        repository_dir,
        target_dir: &target_dir,
        cli_package: fixture.cli_package.as_deref(),
    };

    let failures: Vec<String> = fixture
        .runs
        .iter()
        .filter_map(|run| drive(&context, run, &stored))
        .collect();
    assert!(failures.is_empty(), "\n\n{}", failures.join("\n"));
}

/// Everything a run needs that is the same for every run over one
/// fixture.
struct Context<'a> {
    name: &'a str,
    fixture_dir: &'a Path,
    repository_dir: &'a Path,
    target_dir: &'a Path,
    cli_package: Option<&'a str>,
}

/// Materialise the project, drive it through one run, and hand back
/// the failure message where the run came out short.
fn drive(context: &Context, run: &Run, stored: &Snapshot) -> Option<String> {
    let mut report = Report::new(context.name.to_owned(), run.name.clone());
    let project = TempDir::new().expect("create a directory for the fixture's project");
    let dir = project.path();
    snapshot::write(dir, stored);
    write_dylint_toml(dir, context, run);
    report.record(stored.clone());

    let mut previous = stored.clone();
    let mut reported = collect(dir, context, &mut report);
    for number in 1..=run.rounds {
        check_handed(context, run, number, &reported, &mut report);
        apply(dir, &previous, context.target_dir, &mut report, number);
        let current = snapshot::read(dir);
        report.record(current.clone());
        check_shape(context, run, number, (&previous, &current), &mut report);
        previous = current;
        reported = collect(dir, context, &mut report);
    }
    check_settled(run, &reported, &mut report);

    expect_success(round::test(dir, context.target_dir), &mut report);
    check_output(dir, context, &mut report);
    report.finish()
}

/// Write the project's `dylint.toml`: the plugin's location, then the
/// run's own configuration appended to it. The run's file holds only
/// the configuration, since where the plugin sits is this checkout's
/// business rather than the fixture's.
fn write_dylint_toml(project_dir: &Path, context: &Context, run: &Run) {
    let configuration_file = run.directory(context.fixture_dir).join("dylint.toml");
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
///
/// `project` is the project as the round receives it, whose files the
/// round opens by restamping.
fn apply(
    project_dir: &Path,
    project: &Snapshot,
    target_dir: &Path,
    report: &mut Report,
    number: usize,
) {
    round::restamp(project_dir, project);
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
    run: &Run,
    number: usize,
    rounds: (&Snapshot, &Snapshot),
    report: &mut Report,
) {
    let (before, after) = rounds;
    let file = run
        .directory(context.fixture_dir)
        .join(format!("round-{number}.diff"));
    let what = format!("[round {number}] the shape the round left");
    if let Err(failure) = expectation::check(&file, &diff::unified(before, after), &what) {
        report.fail(failure);
    }
}

/// Every diagnostic the project reports as it now stands, from both
/// linters.
fn collect(project_dir: &Path, context: &Context, report: &mut Report) -> Reported {
    let mut reported = Reported::default();
    for check in [round::dylint_check, round::clippy_check] {
        let outcome = check(project_dir, context.target_dir);
        reported.extend(&outcome.stdout, project_dir);
        expect_success(outcome, report);
    }
    reported
}

/// Hold the round to the diagnostics the fixture stored as what it
/// was handed, which is what its stored diff is the answer to.
fn check_handed(
    context: &Context,
    run: &Run,
    number: usize,
    reported: &Reported,
    report: &mut Report,
) {
    let file = run
        .directory(context.fixture_dir)
        .join(format!("round-{number}.stderr"));
    let what = format!("[round {number}] the diagnostics the round was handed");
    if let Err(failure) = expectation::check(&file, &reported.render(), &what) {
        report.fail(failure);
    }
}

/// Hold the project to having settled once the last round has run,
/// or, where the run declares it cannot settle, to still reporting
/// the shapes it was written around.
///
/// Nothing is stored for this, deliberately. Every other expectation
/// here is re-recorded by a blessing run, so a regression that left
/// findings behind after the last round would be written into the
/// file and pass. This is the one claim a blessing run cannot
/// launder.
fn check_settled(run: &Run, reported: &Reported, report: &mut Report) {
    if run.settles && !reported.lints.is_empty() {
        report.fail(format!(
            "the project is not settled after its last round; still reported:\n{}",
            reported.render(),
        ));
    }
    if !run.settles && reported.lints.is_empty() {
        report.fail(
            "the run declares that the project does not settle, but nothing is \
             reported after its last round; either the project no longer holds a \
             shape the rules decline, or the declaration is stale"
                .to_owned(),
        );
    }
}

/// Hold the fixture's `main`, run over the settled project, to the
/// output the fixture stored for it. A rewrite that changed what the
/// project prints is the failure this reports.
///
/// A `main` that prints nothing would pass against an equally empty
/// stored file, so the empty case is a failure of the fixture rather
/// than a pass.
fn check_output(project_dir: &Path, context: &Context, report: &mut Report) {
    let Some(package) = context.cli_package else {
        return;
    };
    let outcome = round::run_cli(project_dir, context.target_dir, package);
    if !outcome.success {
        expect_success_when(outcome, report, "after the last round");
        return;
    }
    if outcome.stdout.is_empty() {
        report.fail(
            "the fixture's `main` printed nothing, so holding it to a stored file \
             would assert nothing; have it print what each crate in the project \
             contributes"
                .to_owned(),
        );
        return;
    }
    let file = context.fixture_dir.join(OUTPUT);
    let what = "the output of the fixture's `main`";
    if let Err(failure) = expectation::check(&file, &outcome.stdout, what) {
        report.fail(failure);
    }
}

fn expect_success(outcome: round::Outcome, report: &mut Report) {
    expect_success_when(outcome, report, "");
}

/// Record a failed invocation, quoting what it printed. `when` names
/// the moment in the run where that is not already obvious from the
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

/// The build cache the fixture `name` owns. Another fixture
/// declaring the same dependencies shares it, which
/// [`Fixture::cache`] resolves.
pub fn cache_dir(repository_dir: &Path, name: &str) -> PathBuf {
    repository_dir.join(CACHE).join(name)
}

/// Every fixture in this checkout, in ascending order.
///
/// Read off disk rather than listed, so it cannot go stale. The tests
/// name their fixtures one by one instead, which is what gives each a
/// failure of its own and lets two run at once; a fixture nobody
/// named is one nothing drives, and is for review to catch.
pub fn fixtures(repository_dir: &Path) -> Vec<String> {
    let fixtures_dir = repository_dir.join(FIXTURES);
    let entries = fs::read_dir(&fixtures_dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", fixtures_dir.display()));
    let mut names: Vec<String> = entries
        .map(|entry| entry.expect("read an entry of the fixture directory"))
        .filter(|entry| entry.path().join("fixture.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Fill the build cache the fixture `name` uses, so a later run pays
/// for the standard library, the registry dependencies and the plugin
/// from the cache rather than from cold.
///
/// Everything a round compiles is compiled here: each linter has a
/// wrapper of its own, so each gets a cache lane of its own, and the
/// project's own tests and `main` are built by plain rustc, which is
/// a third. A fix pass is not run, so nothing the warmup does depends
/// on which run comes first; the first run's configuration is written
/// only because a check needs some configuration to read.
pub fn warm(repository_dir: &Path, name: &str) {
    let fixtures_dir = repository_dir.join(FIXTURES);
    let fixture_dir = fixtures_dir.join(name);
    let fixture = Fixture::load(&fixtures_dir, name);
    let target_dir = cache_dir(repository_dir, fixture.cache(name));
    let context = Context {
        name,
        fixture_dir: &fixture_dir,
        repository_dir,
        target_dir: &target_dir,
        cli_package: fixture.cli_package.as_deref(),
    };
    let project = TempDir::new().expect("create a directory for the fixture's project");
    let dir = project.path();
    snapshot::write(dir, &snapshot::read(&fixture_dir.join("project")));
    let first = fixture.runs.first().expect("a fixture declares a run");

    let mut report = Report::new(name.to_owned(), first.name.clone());
    write_dylint_toml(dir, &context, first);
    for check in [round::dylint_check, round::clippy_check] {
        expect_success(check(dir, &target_dir), &mut report);
    }
    expect_success(round::build_tests(dir, &target_dir), &mut report);
    if let Some(failure) = report.finish() {
        panic!("\n{failure}");
    }
}
