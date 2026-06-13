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
//! - `html` writes the `index.html` GitHub Pages reads (the
//!   project's public catalogue) plus the sibling assets it links —
//!   one file per stylesheet and the navigation script.
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

use crate::check_md::{CheckOutcome, check_rules_dir, write_rules_dir};
use crate::extract::collect_rules;
use crate::model::{RenderContext, Rule};
use crate::render::markdown::HIGHLIGHT_CSS;
use crate::render::{
    CANTARELL_LICENSE, CANTARELL_LICENSE_FILENAME, CONFIG_TOGGLE_SCRIPT,
    CONFIG_TOGGLE_SCRIPT_FILENAME, FONT_ASSETS, HIGHLIGHT_CSS_DARK_FILENAME,
    HIGHLIGHT_CSS_LIGHT_FILENAME, NAV_TOGGLE_SCRIPT, NAV_TOGGLE_SCRIPT_FILENAME, RULE_ANCHOR_ICON,
    RULE_ANCHOR_ICON_FILENAME, STYLESHEETS, THEME_ICONS, THEME_TOGGLE_SCRIPT,
    THEME_TOGGLE_SCRIPT_FILENAME, render_page,
};
use cargo_toml::Manifest;
use clap::{Parser, Subcommand};
use command_extra::CommandExtra;
use pipe_trait::Pipe;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

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
    /// Render the catalogue to an HTML page plus its sibling CSS/JS
    /// assets.
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
    /// plus a README.md index. Files that no longer correspond to
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
    // Derive the human-facing repository URL from Cargo.toml so a
    // fork picks up its own URL without hand-editing the renderer.
    // Cargo's `repository` field typically ends in `.git` for clone
    // ergonomics; strip it for the human-facing URL.
    let repo_url = manifest
        .package
        .as_ref()
        .and_then(|package| package.repository.as_ref().and_then(|repo| repo.get().ok()))
        .map(|url| url.strip_suffix(".git").unwrap_or(url).to_owned())
        .unwrap_or_else(|| "https://github.com/KSXGitHub/perfectionist".to_owned());

    let rules = match load_rules(root) {
        Ok(rules) => rules,
        Err(code) => return code,
    };

    fs::create_dir_all(out_dir).expect("failed to create output directory");
    let context = RenderContext {
        git_ref,
        commit_sha: &commit_sha,
        repo_url: &repo_url,
    };
    let html = render_page(&rules, &context);
    let index_path = out_dir.join("index.html");
    fs::write(&index_path, html).expect("failed to write index.html");

    // Each stylesheet lands beside index.html as its own file, linked
    // individually by the page rather than inlined or concatenated.
    for (name, content) in STYLESHEETS {
        let path = out_dir.join(name);
        fs::write(&path, content).unwrap_or_else(|error| panic!("failed to write {name}: {error}"));
    }
    // The syntax-highlighting CSS is generated at runtime by syntect
    // (both sheets from one theme-set load), so it's written from the
    // live strings rather than `include_str!`. The dark variant is
    // linked after the light one.
    fs::write(
        out_dir.join(HIGHLIGHT_CSS_LIGHT_FILENAME),
        &HIGHLIGHT_CSS.light,
    )
    .expect("failed to write light highlight CSS");
    fs::write(
        out_dir.join(HIGHLIGHT_CSS_DARK_FILENAME),
        &HIGHLIGHT_CSS.dark,
    )
    .expect("failed to write dark highlight CSS");
    // The page scripts, loaded via `<script src>`.
    fs::write(out_dir.join(NAV_TOGGLE_SCRIPT_FILENAME), NAV_TOGGLE_SCRIPT)
        .expect("failed to write nav script");
    fs::write(
        out_dir.join(THEME_TOGGLE_SCRIPT_FILENAME),
        THEME_TOGGLE_SCRIPT,
    )
    .expect("failed to write theme script");
    fs::write(
        out_dir.join(CONFIG_TOGGLE_SCRIPT_FILENAME),
        CONFIG_TOGGLE_SCRIPT,
    )
    .expect("failed to write config-toggle script");

    // Lands beside index.html so the stylesheet's relative `url(...)`
    // resolves.
    let icon_path = out_dir.join(RULE_ANCHOR_ICON_FILENAME);
    fs::write(&icon_path, RULE_ANCHOR_ICON).expect("failed to write rule-anchor icon");

    // The colour-scheme icons, referenced as CSS masks by settings.css.
    for (name, content) in THEME_ICONS {
        let path = out_dir.join(name);
        fs::write(&path, content).unwrap_or_else(|error| panic!("failed to write {name}: {error}"));
    }

    // The Cantarell body-text webfont, referenced by the `@font-face`
    // `url(...)` fallback in base.css, plus its SIL OFL 1.1 license.
    for (name, bytes) in FONT_ASSETS {
        let path = out_dir.join(name);
        fs::write(&path, bytes).unwrap_or_else(|error| panic!("failed to write {name}: {error}"));
    }
    fs::write(out_dir.join(CANTARELL_LICENSE_FILENAME), CANTARELL_LICENSE)
        .expect("failed to write Cantarell font license");

    eprintln!("wrote {} rule(s) to {}", rules.len(), index_path.display());
    ExitCode::SUCCESS
}

/// Compute the relative-path prefix the markdown renderer needs
/// to climb from a file under `rules_dir` back up to the repo
/// `root`. Returned with a trailing `/`, so callers can append a
/// path under `root` directly (e.g. `"../" + "src/rules/foo.rs"`).
///
/// Returns `Err` when `rules_dir` is a symbolic link (which could
/// resolve outside `root` and let write-md's orphan-delete step
/// touch files outside the repo), when it's the same directory as
/// `root` (the catalogue's `README.md` would collide with the
/// project's own), when it resolves to a path outside `root`, or
/// when the path contains `..` segments — `std::path::absolute`
/// doesn't normalise those, so `rules/../rules` would otherwise be
/// (silently) interpreted as depth 3 and produce broken links.
fn source_link_prefix_for(rules_dir: &Path, root: &Path) -> Result<String, String> {
    use std::path::Component;

    // Symlink guard. `std::path::absolute` is purely lexical
    // (doesn't follow symlinks), so a `rules_dir` that's itself a
    // symlink to `/etc` would pass the `strip_prefix` containment
    // check below — and then `write-md`'s orphan-delete loop
    // would happily remove `.md` files via the symlink target.
    // Refuse before any filesystem mutation. Only checked when
    // the path exists; a non-existent `rules_dir` (check-md
    // against a never-created location) can't be a symlink, and
    // write-md creates the directory with `create_dir_all`,
    // which won't follow a symlink to create a regular dir.
    if let Ok(metadata) = std::fs::symlink_metadata(rules_dir)
        && metadata.file_type().is_symlink()
    {
        return Err(format!(
            "rules_dir `{}` is a symbolic link; refusing to operate on it. \
             Pass the resolved target directly so write-md's orphan-delete \
             step can't follow the link out of `--root`",
            rules_dir.display(),
        ));
    }

    let abs_root = std::path::absolute(root)
        .map_err(|error| format!("failed to resolve --root `{}`: {error}", root.display()))?;
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

    // Walk the relative components by hand instead of using
    // `components().count()`: `std::path::absolute` does *not*
    // collapse `..` or `.`, so `rules/../rules` and `rules/sub/..`
    // would both report depth ≥ 3 — producing a `Source:` link
    // three directories too deep. Reject `..` outright (the user
    // should write the path they mean) and skip `.` (semantically
    // a no-op).
    let mut depth = 0usize;
    for component in rel.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!(
                    "rules_dir `{}` contains a `..` segment; write the resolved path \
                     directly (e.g. `rules/` rather than `rules/sub/..`)",
                    rules_dir.display(),
                ));
            }
            // `RootDir` / `Prefix` can't appear here: `strip_prefix`
            // returns only the suffix past `abs_root`, which is
            // necessarily relative.
            _ => {
                return Err(format!(
                    "rules_dir `{}` contains an unsupported path component {component:?}",
                    rules_dir.display(),
                ));
            }
        }
    }
    if depth == 0 {
        // rules_dir == root (or only `.` components): the
        // catalogue's `README.md` would overwrite the project's
        // own README, and per-rule files would litter the repo
        // root. Refuse before any write.
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

#[cfg(test)]
mod tests;
