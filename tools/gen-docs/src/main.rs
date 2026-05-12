//! Generate documentation for perfectionist's implemented lints.
//!
//! Reads each `src/rules/*.rs` (skipping `mod.rs`-style index files
//! that don't declare a lint), locates the `declare_tool_lint!`
//! invocation, and pulls four things out of it: the lint identifier
//! (`pub perfectionist::NAME`), its default level, its one-line
//! description, and the `///` doc-comment block that documents it.
//! In addition, when a rule's file defines a `Config` struct paired
//! with a `CONFIG_KEY` constant, the configurable fields and any
//! non-built-in types they reference (enums, project-local structs)
//! are surfaced too.
//!
//! The same extracted model feeds three output modes, selected via
//! subcommand:
//!
//! - `html` writes the self-contained `index.html` GitHub Pages
//!   reads (the project's public catalogue).
//! - `write-md` writes a `rules/` directory with one markdown file
//!   per rule plus a `README.md` index, intended for in-repo
//!   browsing alongside `src/rules/` and `planned-rules/`.
//! - `check-md` re-renders that same directory in memory and
//!   compares it against the on-disk copy, failing the build if
//!   anything drifts (missing files, orphan files, content
//!   mismatch). Wired into CI so the in-repo markdown stays in
//!   lockstep with the rule sources.

mod check_md;
mod extract;
mod model;
mod render;
mod render_md;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use cargo_toml::{Inheritable, Manifest};
use clap::{Parser, Subcommand};
use command_extra::CommandExtra;
use pipe_trait::Pipe;

use crate::check_md::{CheckOutcome, check_rules_dir, write_rules_dir};
use crate::extract::collect_rules;
use crate::model::{RenderContext, Rule};
use crate::render::render_page;

#[derive(Parser)]
#[clap(about = "Render perfectionist's lint catalogue")]
struct Cli {
    #[clap(
        long,
        default_value = ".",
        global = true,
        help = "Repository root containing Cargo.toml and src/rules/"
    )]
    root: PathBuf,

    #[clap(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Render the catalogue to a self-contained HTML page.
    Html {
        #[clap(help = "Output directory; index.html will be written here")]
        out_dir: PathBuf,

        #[clap(
            long,
            default_value = "master",
            value_parser = clap::builder::NonEmptyStringValueParser::new(),
            help = r#"Git ref the rendered "Source:" links should target; resolved to a commit SHA via `git rev-parse` so the links are permalinks"#,
        )]
        git_ref: String,
    },

    /// Check whether the on-disk markdown copy of the catalogue is
    /// up to date with the rule sources. Exits non-zero if anything
    /// drifts (content mismatch, missing file, orphan file).
    CheckMd {
        #[clap(help = "Directory containing one markdown file per rule, plus a README.md index")]
        rules_dir: PathBuf,
    },

    /// Write the markdown copy of the catalogue, one file per rule
    /// plus a `README.md` index. Files that no longer correspond to
    /// any rule are removed.
    WriteMd {
        #[clap(help = "Directory to write the markdown catalogue into; created if missing")]
        rules_dir: PathBuf,
    },
}

fn resolve_git_ref(root: &Path, git_ref: &str) -> String {
    let revision = format!("{git_ref}^{{commit}}");
    let output = "git"
        .pipe(Command::new)
        .with_current_dir(root)
        .with_arg("rev-parse")
        .with_arg("--verify")
        .with_arg(&revision)
        .output()
        .expect("failed to invoke `git rev-parse`");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "`git rev-parse --verify {revision}` failed ({}): {}",
            output.status,
            stderr.trim(),
        );
    }
    output
        .stdout
        .pipe(String::from_utf8)
        .expect("`git rev-parse` produced non-UTF-8 output")
        .trim()
        .to_owned()
}

/// Collect the rule list every subcommand needs. Centralised here
/// so the "no rules found" guard, the sort, and the rule-source
/// path all have a single home; the subcommands focus on what to
/// do *with* the list, not how to produce it.
fn load_rules(root: &Path) -> Result<Vec<Rule>, ExitCode> {
    let rules_src_dir = root.join("src").join("rules");
    let mut rules = collect_rules(&rules_src_dir);
    rules.sort_by(|a, b| a.namespaced.cmp(&b.namespaced));
    if rules.is_empty() {
        eprintln!("no rules found under {}", rules_src_dir.display());
        return Err(ExitCode::FAILURE);
    }
    Ok(rules)
}

fn run_html(root: &Path, out_dir: &Path, git_ref: &str) -> ExitCode {
    // Resolve the user-supplied ref (typically a branch like `master`)
    // to a commit SHA so the rendered "Source:" links are permalinks
    // that survive future commits to the branch. The original ref is
    // kept for the page title and banner, which want to read as
    // "Showing docs for `master`" rather than a bare SHA.
    let commit_sha = resolve_git_ref(root, git_ref);

    let manifest = Manifest::from_path(root.join("Cargo.toml")).expect("failed to read Cargo.toml");
    let crate_version = manifest
        .package
        .as_ref()
        .and_then(|package| match &package.version {
            Inheritable::Set(value) => Some(value.clone()),
            Inheritable::Inherited => None,
        })
        .unwrap_or_else(|| "unknown".to_owned());
    // Derive the human-facing repository URL from Cargo.toml so a
    // fork picks up its own URL without hand-editing the renderer.
    // Cargo's `repository` field typically ends in `.git` for clone
    // ergonomics; strip it for the human-facing URL.
    let repo_url = manifest
        .package
        .as_ref()
        .and_then(|package| package.repository.as_ref().and_then(|r| r.get().ok()))
        .map(|url| url.strip_suffix(".git").unwrap_or(url).to_owned())
        .unwrap_or_else(|| "https://github.com/KSXGitHub/perfectionist".to_owned());

    let rules = match load_rules(root) {
        Ok(rules) => rules,
        Err(code) => return code,
    };

    fs::create_dir_all(out_dir).expect("failed to create output directory");
    let context = RenderContext {
        crate_version: &crate_version,
        git_ref,
        commit_sha: &commit_sha,
        repo_url: &repo_url,
    };
    let html = render_page(&rules, &context);
    let index_path = out_dir.join("index.html");
    fs::write(&index_path, html).expect("failed to write index.html");

    eprintln!("wrote {} rule(s) to {}", rules.len(), index_path.display());
    ExitCode::SUCCESS
}

/// Compute the relative-path prefix the markdown renderer needs
/// to climb from a file under `rules_dir` back up to the repo
/// `root`. Returned with a trailing `/`, so callers can append a
/// path under `root` directly (e.g. `"../" + "src/rules/foo.rs"`).
///
/// Returns `Err` when `rules_dir` is the same directory as `root`
/// (the catalogue's `README.md` would collide with the project's
/// own) or when it resolves to a path outside `root` (the renderer
/// has no way to produce a correct relative link in that case).
fn source_link_prefix_for(rules_dir: &Path, root: &Path) -> Result<String, String> {
    let abs_root = std::path::absolute(root)
        .map_err(|error| format!("failed to resolve --root `{}`: {error}", root.display(),))?;
    let abs_rules = std::path::absolute(rules_dir).map_err(|error| {
        format!(
            "failed to resolve rules_dir `{}`: {error}",
            rules_dir.display(),
        )
    })?;
    let rel = abs_rules.strip_prefix(&abs_root).map_err(|_| {
        format!(
            "rules_dir `{}` is not inside --root `{}`; the markdown renderer can only \
             build relative `Source:` links when rules_dir is under the repo root",
            rules_dir.display(),
            root.display(),
        )
    })?;
    let depth = rel.components().count();
    if depth == 0 {
        // rules_dir == root: the catalogue's `README.md` would
        // overwrite the project's own README, and per-rule files
        // would litter the repo root. Refuse before any write.
        return Err(format!(
            "rules_dir `{}` resolves to the same directory as --root `{}`; \
             choose a subdirectory (e.g. `rules/`) so the catalogue's `README.md` \
             doesn't collide with the project's own",
            rules_dir.display(),
            root.display(),
        ));
    }
    Ok("../".repeat(depth))
}

fn run_check_md(root: &Path, rules_dir: &Path) -> ExitCode {
    let rules = match load_rules(root) {
        Ok(rules) => rules,
        Err(code) => return code,
    };
    let source_link_prefix = match source_link_prefix_for(rules_dir, root) {
        Ok(prefix) => prefix,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };
    match check_rules_dir(&rules, rules_dir, &source_link_prefix) {
        CheckOutcome::Clean => {
            eprintln!(
                "{} is up to date ({} rule(s))",
                rules_dir.display(),
                rules.len(),
            );
            ExitCode::SUCCESS
        }
        CheckOutcome::Drifted(report) => {
            eprint!("{report}");
            eprintln!(
                "{} is out of date. Run `just gen-rules-md` to regenerate.",
                rules_dir.display(),
            );
            ExitCode::FAILURE
        }
    }
}

fn run_write_md(root: &Path, rules_dir: &Path) -> ExitCode {
    let rules = match load_rules(root) {
        Ok(rules) => rules,
        Err(code) => return code,
    };
    let source_link_prefix = match source_link_prefix_for(rules_dir, root) {
        Ok(prefix) => prefix,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };
    let summary = write_rules_dir(&rules, rules_dir, &source_link_prefix);
    eprintln!(
        "{}: rewrote {} rule file(s); index {}; {} orphan(s) removed",
        rules_dir.display(),
        summary.rules_changed,
        if summary.index_changed {
            "rewritten"
        } else {
            "unchanged"
        },
        summary.orphans_removed,
    );
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        CliCommand::Html { out_dir, git_ref } => run_html(&cli.root, &out_dir, &git_ref),
        CliCommand::CheckMd { rules_dir } => run_check_md(&cli.root, &rules_dir),
        CliCommand::WriteMd { rules_dir } => run_write_md(&cli.root, &rules_dir),
    }
}
