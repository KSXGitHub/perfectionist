//! Generate the static documentation site for perfectionist's
//! implemented lints.
//!
//! Reads each `src/rules/*.rs` (skipping `mod.rs`-style index files
//! that don't declare a lint), locates the `declare_tool_lint!`
//! invocation, and pulls four things out of it: the lint identifier
//! (`pub perfectionist::NAME`), its default level, its one-line
//! description, and the `///` doc-comment block that documents it.
//! In addition, when a rule's file defines a `Config` struct paired
//! with a `CONFIG_KEY` constant, the configurable fields and any
//! non-built-in types they reference (enums, project-local structs)
//! are surfaced too. The output is a single self-contained
//! `index.html` written into the directory passed on the command
//! line.

mod extract;
mod model;
mod render;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use cargo_toml::{Inheritable, Manifest};
use clap::Parser;
use command_extra::CommandExtra;
use pipe_trait::Pipe;

use crate::extract::collect_rules;
use crate::model::RenderContext;
use crate::render::render_page;

#[derive(Parser)]
#[clap(about = "Render perfectionist's lint catalogue to a static HTML page")]
struct Cli {
    #[clap(help = "Repository root containing Cargo.toml and src/rules/")]
    root: PathBuf,

    #[clap(help = "Output directory; index.html will be written here")]
    out_dir: PathBuf,

    #[clap(
        long,
        default_value = "master",
        value_parser = clap::builder::NonEmptyStringValueParser::new(),
        help = r#"Git ref the rendered "Source:" links should target; resolved to a commit SHA via `git rev-parse` so the links are permalinks"#,
    )]
    git_ref: String,
}

fn resolve_git_ref(root: &Path, git_ref: &str) -> String {
    let output = "git"
        .pipe(Command::new)
        .with_current_dir(root)
        .with_arg("rev-parse")
        .with_arg("--verify")
        .with_arg(format!("{git_ref}^{{commit}}"))
        .output()
        .expect("failed to invoke `git rev-parse`");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("`git rev-parse {git_ref}` failed: {}", stderr.trim());
    }
    let sha = output
        .stdout
        .pipe(String::from_utf8)
        .expect("`git rev-parse` produced non-UTF-8 output")
        .trim()
        .to_owned();
    assert!(
        !sha.is_empty(),
        "`git rev-parse {git_ref}` produced empty output",
    );
    sha
}

fn main() -> ExitCode {
    let Cli {
        root,
        out_dir,
        git_ref,
    } = Cli::parse();

    // Resolve the user-supplied ref (typically a branch like `master`)
    // to a commit SHA so the rendered "Source:" links are permalinks
    // that survive future commits to the branch. The original ref is
    // kept for the page title and banner, which want to read as
    // "Showing docs for `master`" rather than a bare SHA.
    let commit_sha = resolve_git_ref(&root, &git_ref);

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

    let rules_dir = root.join("src").join("rules");
    let mut rules = collect_rules(&rules_dir);
    rules.sort_by(|a, b| a.namespaced.cmp(&b.namespaced));

    if rules.is_empty() {
        eprintln!("no rules found under {}", rules_dir.display());
        return ExitCode::FAILURE;
    }

    fs::create_dir_all(&out_dir).expect("failed to create output directory");
    let context = RenderContext {
        crate_version: &crate_version,
        git_ref: &git_ref,
        commit_sha: &commit_sha,
        repo_url: &repo_url,
    };
    let html = render_page(&rules, &context);
    let index_path = out_dir.join("index.html");
    fs::write(&index_path, html).expect("failed to write index.html");

    eprintln!("wrote {} rule(s) to {}", rules.len(), index_path.display());
    ExitCode::SUCCESS
}
