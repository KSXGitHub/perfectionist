use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::emit_at_enclosing_hir;
use crate::markdown::{ClassifyOptions, classify_constructs};
use crate::module_reparse::parse_crate_module_files;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

mod collect;
mod config;

use collect::{NodeState, collect_doc_nodes};
use config::{Config, ConstructCategory, ResolvedConfig};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Forbids markdown-specific constructs in a doc comment that
    /// clap's derive macros consume as `--help` text — HTML tags,
    /// inline / reference / intra-doc links, code blocks, code spans,
    /// and headings. The rule fires on the doc comment of a struct or
    /// enum deriving `clap::Parser`, `Args`, `Subcommand`, `ValueEnum`,
    /// or `CommandFactory`, and on the doc comments of their fields and
    /// variants.
    ///
    /// A `ValueEnum`'s own type-level doc comment is left alone: unlike
    /// its variant docs — clap's per-value help — it never reaches
    /// `--help`.
    ///
    /// Bold, italics, and lists are not flagged by default — clap
    /// renders them as their literal characters, which usually reads
    /// cleanly — but are available through the `extra_constructs` knob.
    ///
    /// The lint stays silent on a node that overrides its help text
    /// with a plain string (`#[arg(help = "...")]`,
    /// `#[command(about = "...")]`, ...), because the doc comment is
    /// then no longer the source of truth for `--help`. A node marked
    /// `#[clap(verbatim_doc_comment)]` instead gets a softer note that
    /// the markdown will appear verbatim in the terminal.
    ///
    /// A project that never wants a doc comment to become `--help` text
    /// at all — preferring an explicit override on every command,
    /// argument, and value — can opt into the stricter
    /// `require_help_override` mode. It flags every clap-derived doc
    /// comment that reaches `--help` without such an override, markdown
    /// or not, and reports each as a single missing-override finding
    /// rather than per markdown construct. A node with no doc comment is
    /// still left alone. This mode is off by default.
    ///
    /// ### Why is this bad?
    ///
    /// By default, clap does **not** render doc comments through a
    /// markdown processor. The raw text is shown verbatim in the
    /// terminal `--help` output. Writing `` [`PathBuf`] `` produces a
    /// docs.rs link in HTML output but shows literally as
    /// `` [`PathBuf`] `` in the terminal — a classic two-audience leak.
    /// The doc comment serves both `cargo doc` readers and `--help`
    /// readers, and markdown that helps the former actively degrades
    /// the latter.
    ///
    /// The escape hatch is to override the help text with a plain
    /// string, keeping the rich doc comment for `cargo doc`:
    ///
    /// ```rust,ignore
    /// /// Builds the lockfile by walking [`Dependency`] graphs.
    /// #[arg(help = "Builds the lockfile by walking dependency graphs.")]
    /// pub deps: PathBuf,
    /// ```
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// #[derive(clap::Parser)]
    /// struct Cli {
    ///     /// Path to the [`PackageManifest`].
    ///     ///
    ///     /// See [the manifest format](https://example.com/manifest).
    ///     manifest: PathBuf,
    /// }
    /// ```
    ///
    /// **Prefer:** (no markdown in the help text)
    ///
    /// ```rust,ignore
    /// #[derive(clap::Parser)]
    /// struct Cli {
    ///     /// Path to the package manifest.
    ///     manifest: PathBuf,
    /// }
    /// ```
    pub perfectionist::CLAP_HELP_MARKDOWN,
    Warn,
    "markdown construct in a clap-derived doc comment leaks into `--help` output",
    report_in_external_macro: false
}

/// Active by default. Read by [`register_pass`] below; gen-docs picks
/// the constant up via syn to render the rule's default state.
pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Active;

const CONFIG_KEY: &str = "perfectionist::clap_help_markdown";

pub struct ClapHelpMarkdown {
    config: ResolvedConfig,
}

impl ClapHelpMarkdown {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            config: ResolvedConfig::from_config(config),
        }
    }
}

impl_lint_pass!(ClapHelpMarkdown => [CLAP_HELP_MARKDOWN]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[CLAP_HELP_MARKDOWN]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("clap_help_markdown", DEFAULT_STATE) {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(ClapHelpMarkdown::new()));
}

/// One parked finding, resolved to its enclosing HIR node and emitted in
/// `check_crate_post`.
enum Violation {
    /// A forbidden markdown construct in a clap-derived doc comment that
    /// feeds `--help`: which construct category, whether the node opted
    /// into verbatim rendering (softer note, no autofix), and the
    /// code-span replacement text when the trivial `` `Foo` `` → `Foo`
    /// autofix applies.
    Markdown {
        category: ConstructCategory,
        soft: bool,
        replacement: Option<String>,
    },
    /// The `require_help_override` mode fired: a clap-derived doc comment
    /// reaches `--help` with no explicit override. One per node.
    MissingOverride,
}

impl<'tcx> LateLintPass<'tcx> for ClapHelpMarkdown {
    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        // Re-parse the crate's module files so the `#[derive(...)]`,
        // `#[arg(...)]`, and doc-comment attributes survive (macro
        // expansion has consumed `#[derive]` by the late pass) and every
        // separate-file submodule is reached.
        let (crates, live_module_spans) = parse_crate_module_files(cx);
        let doc_nodes = collect_doc_nodes(&crates, &live_module_spans);
        if doc_nodes.is_empty() {
            return;
        }

        let mut violations: Vec<(Span, Violation)> = Vec::new();
        walk_local_comments(cx, |chunk| match chunk.surface {
            CommentSurface::DocBlock | CommentSurface::DocBlockBlock => {
                let offset = chunk.source_span.lo().0;
                if self.config.require_help_override {
                    // Stricter mode: every help-bound doc comment must be
                    // replaced by an explicit override, so flag each node
                    // once at its opening `///` line. This supersedes the
                    // markdown scan — an override silences that concern
                    // anyway, and `first_lines` holds exactly the
                    // doc-commented, unoverridden nodes.
                    if doc_nodes.first_lines.contains(&offset)
                        && let Some(span) = first_doc_line_span(chunk)
                    {
                        violations.push((span, Violation::MissingOverride));
                    }
                } else if let Some(state) = doc_nodes.by_line.get(&offset) {
                    // A doc block belongs to a clap-bound node when its
                    // start offset matches one of the node's `///` lines;
                    // absent ⇒ not a clap node (or overridden) ⇒ skip.
                    self.scan_chunk(chunk, *state, &mut violations);
                }
            }
            CommentSurface::PlainLine | CommentSurface::PlainBlock => {}
        });

        emit_at_enclosing_hir(cx.tcx, violations, |hir_id, span, violation| {
            emit(cx, hir_id, span, &violation);
        });
    }
}

impl ClapHelpMarkdown {
    fn scan_chunk(
        &self,
        chunk: &CommentChunk<'_>,
        state: NodeState,
        out: &mut Vec<(Span, Violation)>,
    ) {
        let options = ClassifyOptions {
            detect_emphasis: self.config.detect_emphasis(),
            detect_lists: self.config.detect_lists(),
        };
        let soft = matches!(state, NodeState::Verbatim);
        for found in classify_constructs(&chunk.rendered, options) {
            let Some(category) = ConstructCategory::from_kind(found.kind) else {
                continue;
            };
            if !self.config.forbid.contains(&category) {
                continue;
            }
            // Prefer the precise single-line span (needed for the
            // code-span autofix); fall back to a one-byte anchor at the
            // construct's start for a multi-line construct.
            //
            // No `hir_in_external_macro` guard is needed despite the
            // narrow diagnostic span (see the "Suppressing
            // proc-macro-synthesised violations" convention): the
            // violation text is always real user source. `module_reparse`
            // restricts the walk to on-disk module files, the comment
            // text comes from those files via `walk_local_comments`, and
            // a `///` doc comment cannot be derive-synthesised onto a
            // user-uneditable span — so this rule is not in the vulnerable
            // class. The span is built with `SyntaxContext::root()`, so
            // `report_in_external_macro` never spuriously fires either.
            let precise = chunk.span_for_range(found.range.clone());
            let Some(span) = precise.or_else(|| chunk.span_for(found.range.start, 1)) else {
                continue;
            };
            let replacement = if !soft && category == ConstructCategory::CodeSpan {
                precise.and_then(|_| code_span_autofix(&chunk.rendered[found.range.clone()]))
            } else {
                None
            };
            out.push((
                span,
                Violation::Markdown {
                    category,
                    soft,
                    replacement,
                },
            ));
        }
    }
}

/// The replacement text for the trivial `` `Foo` `` → `Foo` code-span
/// autofix, or `None` when the span is not safe to rewrite
/// machine-applicably.
///
/// Only a single-backtick fence whose content holds no backtick is
/// rewritten. A multi-backtick fence (`` `` `code` `` ``) — used
/// precisely to embed a backtick in the content — would, after stripping
/// only the outer fence, leave a stray backtick behind that re-triggers
/// the lint; those keep the help-only suggestion instead. Per CommonMark,
/// one space on each side is dropped when the span both opens and closes
/// with a space (and is not all spaces).
fn code_span_autofix(span_text: &str) -> Option<String> {
    let inner = span_text.strip_prefix('`')?.strip_suffix('`')?;
    if inner.contains('`') {
        return None;
    }
    let unspaced = inner
        .strip_prefix(' ')
        .and_then(|stripped| stripped.strip_suffix(' '))
        .filter(|stripped| !stripped.is_empty())
        .unwrap_or(inner);
    Some(unspaced.to_owned())
}

/// The source span of the first non-empty rendered line of `chunk`, the
/// anchor for a [`Violation::MissingOverride`] diagnostic.
///
/// A single-line span is used, not the whole (possibly multi-line) doc
/// block: [`emit_at_enclosing_hir`] resolves each diagnostic to the
/// documented node through that node's per-line `#[doc]` attribute spans,
/// and a target spanning several `///` lines is contained by none of them
/// — it would fall back to the crate root and defeat a per-field
/// `#[allow]`. Falls back to the whole-block span only for a doc comment
/// with no content at all (`///` with nothing after it), which cannot
/// carry markdown and is a degenerate case regardless.
fn first_doc_line_span(chunk: &CommentChunk<'_>) -> Option<Span> {
    chunk
        .lines
        .iter()
        .find(|line| line.rendered_len != 0)
        .and_then(|line| chunk.span_for(line.rendered_start, line.rendered_len as u32))
        .or(Some(chunk.source_span))
}

fn emit(cx: &LateContext<'_>, hir_id: hir::HirId, span: Span, violation: &Violation) {
    match violation {
        Violation::Markdown {
            category,
            soft,
            replacement,
        } => emit_markdown(cx, hir_id, span, *category, *soft, replacement.as_deref()),
        Violation::MissingOverride => emit_missing_override(cx, hir_id, span),
    }
}

fn emit_markdown(
    cx: &LateContext<'_>,
    hir_id: hir::HirId,
    span: Span,
    category: ConstructCategory,
    soft: bool,
    replacement: Option<&str>,
) {
    let label = category.label();
    if soft {
        span_lint_hir_and_then(
            cx,
            CLAP_HELP_MARKDOWN,
            hir_id,
            span,
            format!("{label} in a `verbatim_doc_comment` clap doc comment"),
            |diag| {
                diag.note(
                    "clap renders this doc comment verbatim, so the markdown syntax appears \
                     literally in the terminal `--help` output",
                );
            },
        );
        return;
    }
    span_lint_hir_and_then(
        cx,
        CLAP_HELP_MARKDOWN,
        hir_id,
        span,
        format!("{label} in a clap-derived doc comment leaks into `--help` output"),
        |diag| {
            if let Some(replacement) = replacement {
                diag.span_suggestion(
                    span,
                    "remove the code span",
                    replacement.to_owned(),
                    Applicability::MachineApplicable,
                );
            } else {
                diag.help(
                    "override the help text with a plain string \
                     (e.g. `#[arg(help = \"...\")]`) or remove the markdown from the doc comment",
                );
            }
        },
    );
}

fn emit_missing_override(cx: &LateContext<'_>, hir_id: hir::HirId, span: Span) {
    span_lint_hir_and_then(
        cx,
        CLAP_HELP_MARKDOWN,
        hir_id,
        span,
        "clap derives `--help` from this doc comment instead of an explicit override",
        |diag| {
            diag.help(
                "override the help text with a plain string \
                 (e.g. `#[arg(help = \"...\")]` or `#[command(about = \"...\")]`)",
            );
        },
    );
}
