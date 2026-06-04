use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::emit_at_enclosing_hir;
use crate::literal_scan::string_literal_quote_lengths;
use crate::markdown::{SkipRange, scan_code_regions};
use config::{Config, DEFAULT_HOSTS, ForgeKind, Target, glob_match};
use emit::{Violation, emit_diagnostic};
use rustc_ast::LitKind;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{BytePos, Pos, Span};

mod config;
mod emit;
mod parse;
mod scan;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags URLs that reference a file or directory inside a hosted
    /// git repository (GitHub, GitLab, Bitbucket, Codeberg / Gitea,
    /// sourcehut, etc.) when the ref in the URL is a branch or tag rather
    /// than a commit SHA. Scans doc comments, regular comments, and
    /// string literals.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A
    /// branch ref such as `/blob/main/...` resolves to whatever that
    /// branch currently points at, so the linked content can change —
    /// or disappear — without warning after the link is written. A tag
    /// ref is steadier but still not pinned: a tag can be moved, and
    /// the lint can't tell a tag from a branch by name alone (a branch
    /// named `v1.2.3` is valid Git), so it rejects both. A commit SHA
    /// is the only ref that always denotes the exact content the author
    /// linked to.
    ///
    /// This rule only concerns whether the ref is mutable; the
    /// *length* of an accepted SHA is `perfectionist::commit_id_length`'s
    /// concern, and `perfectionist::bare_url` ensures the URL is
    /// wrapped. The three layer rather than overlap.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// /// See <https://github.com/owner/repo/blob/main/src/lib.rs>.
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// /// See <https://github.com/owner/repo/blob/8c1f6e2/src/lib.rs>.
    /// ```
    pub perfectionist::UNPINNED_REPO_REF,
    Warn,
    "repository URL references a branch or tag instead of a commit SHA",
    report_in_external_macro: false
}

use config::CONFIG_KEY;

pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Active;

pub struct UnpinnedRepoRef {
    scan_doc: bool,
    scan_comment: bool,
    scan_string_literal: bool,
    pub(super) sha_recognition_length: usize,
    /// Hostname globs paired with the forge kind they map to, in
    /// configuration order (first match wins).
    hosts: Vec<(String, ForgeKind)>,
    /// Hostname globs to skip outright.
    skip_hosts: Vec<String>,
}

impl UnpinnedRepoRef {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let hosts = if config.hosts.is_empty() {
            // An explicitly empty `hosts = []` would disable the rule
            // entirely; treat it the same as omission and fall back to
            // the built-in list, matching how `bare_url`'s defaults
            // work.
            DEFAULT_HOSTS
                .iter()
                .map(|(hostname, kind)| ((*hostname).to_owned(), *kind))
                .collect()
        } else {
            config
                .hosts
                .into_iter()
                .map(|entry| (entry.hostname, entry.kind))
                .collect()
        };
        Self {
            scan_doc: config.targets.contains(&Target::Doc),
            scan_comment: config.targets.contains(&Target::Comment),
            scan_string_literal: config.targets.contains(&Target::StringLiteral),
            sha_recognition_length: config.sha_recognition_length,
            hosts,
            skip_hosts: config.skip_hosts,
        }
    }

    /// Resolve `host` to its forge kind, or `None` when no configured
    /// host matches or the host is explicitly skipped. `skip_hosts`
    /// takes precedence over `hosts`.
    pub(super) fn forge_kind_for_host(&self, host: &str) -> Option<ForgeKind> {
        if self
            .skip_hosts
            .iter()
            .any(|pattern| glob_match(pattern, host))
        {
            return None;
        }
        self.hosts
            .iter()
            .find(|(pattern, _)| glob_match(pattern, host))
            .map(|(_, kind)| *kind)
    }

    fn scans_any_comment(&self) -> bool {
        self.scan_doc || self.scan_comment
    }

    fn scan_chunk(&self, chunk: &CommentChunk<'_>, out: &mut Vec<(Span, Violation)>) {
        let scan = match chunk.surface {
            CommentSurface::DocBlock | CommentSurface::DocBlockBlock => self.scan_doc,
            CommentSurface::PlainLine | CommentSurface::PlainBlock => self.scan_comment,
        };
        if !scan {
            return;
        }
        // Doc comments are markdown, so a URL inside a code span or
        // code block is example text and must be skipped. Only *code*
        // regions are skipped, not autolinks / labelled links: unlike
        // `bare_url` (which skips an already-wrapped URL), this rule
        // still pins a wrapped-but-mutable ref, so a URL inside
        // `<...>` must be scanned. Plain comments aren't markdown — no
        // skip pass.
        let skips: Vec<SkipRange> = match chunk.surface {
            CommentSurface::DocBlock | CommentSurface::DocBlockBlock => {
                scan_code_regions(&chunk.rendered, true)
            }
            CommentSurface::PlainLine | CommentSurface::PlainBlock => Vec::new(),
        };
        scan::scan_text(
            &chunk.rendered,
            &skips,
            self,
            |offset, length, problem, reference| {
                if let Some(span) = chunk.span_for(offset, length as u32) {
                    out.push((
                        span,
                        Violation {
                            problem,
                            reference: reference.to_owned(),
                        },
                    ));
                }
            },
        );
    }
}

impl_lint_pass!(UnpinnedRepoRef => [UNPINNED_REPO_REF]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[UNPINNED_REPO_REF]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("unpinned_repo_ref", DEFAULT_STATE) {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(UnpinnedRepoRef::new()));
}

impl<'tcx> LateLintPass<'tcx> for UnpinnedRepoRef {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        if !self.scans_any_comment() {
            return;
        }
        let mut violations: Vec<(Span, Violation)> = Vec::new();
        walk_local_comments(lint_context, |chunk| {
            self.scan_chunk(chunk, &mut violations)
        });
        emit_at_enclosing_hir(lint_context.tcx, violations, |hir_id, span, violation| {
            emit_diagnostic(lint_context, hir_id, span, &violation);
        });
    }

    fn check_expr(&mut self, lint_context: &LateContext<'tcx>, expr: &Expr<'tcx>) {
        if !self.scan_string_literal {
            return;
        }
        let ExprKind::Lit(literal) = expr.kind else {
            return;
        };
        if !matches!(literal.node, LitKind::Str(..)) {
            return;
        }
        // No separate `hir_in_external_macro` guard here (cf. the
        // "Suppressing proc-macro-synthesised violations" convention):
        // the scan re-reads source text through `span_to_snippet`
        // below, which is itself the proc-macro defence. A synthesised
        // literal either carries an *expansion* span — filtered by
        // `report_in_external_macro: false`, since the ref span built
        // below inherits `literal.span.ctxt()` — or, in the
        // `clap_derive` shape, a *user-attribute* span whose source
        // text is the attribute, not a string literal, so
        // `string_literal_quote_lengths` returns `None` and the body
        // is never scanned. A `hir_in_external_macro` guard would add
        // nothing testable on top of that, so it is deliberately
        // omitted.
        let Ok(snippet) = lint_context
            .sess()
            .source_map()
            .span_to_snippet(literal.span)
        else {
            return;
        };
        let Some((prefix_length, suffix_length)) = string_literal_quote_lengths(&snippet) else {
            return;
        };
        let body = &snippet[prefix_length..snippet.len() - suffix_length];
        scan::scan_text(body, &[], self, |offset, length, problem, reference| {
            let lo = literal.span.lo() + BytePos::from_u32((prefix_length + offset) as u32);
            let hi = lo + BytePos::from_u32(length as u32);
            let span = Span::new(lo, hi, literal.span.ctxt(), literal.span.parent());
            emit_diagnostic(
                lint_context,
                expr.hir_id,
                span,
                &Violation {
                    problem,
                    reference: reference.to_owned(),
                },
            );
        });
    }
}
