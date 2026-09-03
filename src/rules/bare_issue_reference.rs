use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::emit_at_enclosing_hir;
use crate::markdown::{position_in_skip, scan_skip_regions, utf8_char_len};
use crate::url_scan::back_scan_url_fragment;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir::HirId;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

mod repo_url;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags bare `#NNN` issue / pull-request references in doc
    /// comments (`///`, `//!`) — and, when opted in, in plain `//`
    /// line comments. The autofix rewrites the reference; the
    /// `doc_comment_form` knob selects the shape (inline
    /// `[#123](URL)`, reference `[#123]`, a bare URL, or a `<URL>`
    /// autolink).
    ///
    /// A bare `#NNN` is deeply ambiguous: it might be an issue, a
    /// pull request, a colour like `#123`, or any other numbered
    /// item, so no suggestion is ever `MachineApplicable`. The
    /// `suggest_issue_url` / `suggest_pr_url` knobs choose which link
    /// target(s) the autofix offers — each `MaybeIncorrect` — and
    /// with neither enabled the lint is help-only. The author can
    /// also resolve the ambiguity by enclosing the token in backticks
    /// (so it reads as code) or by using a spelling without a leading
    /// `#`.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A
    /// bare `#123` renders as literal text in CommonMark; only
    /// GitHub's markdown flavour autolinks the token, and only when
    /// the rendering surface is itself within a GitHub repository
    /// view. The link form renders portably across rustdoc, GitHub,
    /// and any other markdown engine.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// /// Closes #123 and supersedes #124.
    /// ```
    ///
    /// **Prefer:** (with
    /// `repository = "https://github.com/owner/repo"` — `forge`
    /// is detected from the host), picking the issue link for one
    /// and the pull-request link for the other:
    ///
    /// ```rust,ignore
    /// /// Closes [#123](https://github.com/owner/repo/issues/123) and
    /// /// supersedes [#124](https://github.com/owner/repo/pull/124).
    /// ```
    pub perfectionist::BARE_ISSUE_REFERENCE,
    Warn,
    "ambiguous bare `#NNN` issue / PR reference in comment",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::bare_issue_reference";

/// Markdown-link shape produced by the autofix inside doc comments.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum DocForm {
    /// `[#123](URL)` — the URL is inlined. Keeps `#123` as the
    /// visible link text.
    #[default]
    Inline,
    /// `[#123]`, with a matching `[#123]: URL` definition appended to
    /// the end of the doc block (after a blank line so it parses as a
    /// definition). In a `/** */` block doc comment the definition
    /// can't be placed safely, so there the fix rewrites only the
    /// `#123` token and leaves the definition to the author.
    Reference,
    /// `https://.../issues/123` — the bare URL replaces the `#123`
    /// token outright (the `#123` text is not kept). NB: in a doc
    /// comment the sibling `perfectionist::bare_url` lint then flags
    /// the substituted URL; pick `bracketed_url` for a form it
    /// accepts.
    BareUrl,
    /// `<https://.../issues/123>` — a markdown autolink replaces the
    /// `#123` token outright. `bare_url` accepts this form.
    BracketedUrl,
}

/// URL shape used inside plain `//` comments when
/// `include_plain_comments = true`.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum PlainForm {
    /// Substitute the URL itself (`https://...`), unwrapped.
    /// Many editors auto-detect a bare URL as clickable. NB: the
    /// sibling `perfectionist::bare_url` lint, whose default also
    /// scans regular comments, will then flag the substituted URL —
    /// pick `bracketed_url` to produce a form both rules accept.
    #[default]
    BareUrl,
    /// Substitute `<https://...>`. The angle-bracket delimiter gives
    /// the URL a clear boundary when it abuts surrounding
    /// punctuation; editors that auto-link URLs typically recognise
    /// it, and `bare_url` accepts it.
    BracketedUrl,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Git-hosting service the repository is on — one of `github`,
    /// `gitlab`, `gitea` — which fixes the issue / PR path layout.
    /// When unset, it is detected from the `repository` host: the
    /// public instances (`github.com`, `gitlab.com`, `codeberg.org`,
    /// `gitea.com`) and the conventional self-hosted subdomains
    /// `gitlab.*`, `github.*`, `gitea.*` and `forgejo.*` all need no
    /// `forge`. Set it explicitly for a self-hosted instance on a
    /// host that gives no such hint (e.g. `git.example.com`). If it is
    /// neither set nor detected from the host, no issue / PR link is
    /// suggested.
    forge: Option<Forge>,
    /// The repository's URL, in any form you'd clone or paste: an
    /// `http(s)://` URL (`"https://github.com/owner/repo"`), an
    /// `ssh://` URL (`"ssh://git@github.com/owner/repo.git"`), or the
    /// scp-like shorthand (`"git@github.com:owner/repo.git"`). No
    /// fixed default.
    repository: Option<String>,
    /// Offer a suggestion that links the reference as an *issue*.
    /// Defaults to `true`.
    suggest_issue_url: bool,
    /// Offer a suggestion that links the reference as a *pull
    /// request*. Defaults to `true`. Ignored on GitLab, where a bare
    /// `#NNN` is always an issue (merge requests are written `!NNN`),
    /// so only the issue suggestion is offered there.
    suggest_pr_url: bool,
    /// Doc-comment fix form: `inline` for `[#N](URL)`, `reference`
    /// for the two-piece `[#N]` + `[#N]: URL` form (the definition is
    /// appended to the doc block; in a `/** */` block doc comment only
    /// the `#N` token is rewritten and the definition is left to the
    /// author). Defaults to `inline`. Ignored for plain-comment fixes
    /// — those follow `plain_comment_form` instead.
    doc_comment_form: DocForm,
    /// When `true`, also lint plain `//` line comments. The
    /// autofix in plain comments uses `plain_comment_form`'s URL
    /// shape (since plain comments aren't markdown). Plain block
    /// comments (`/* ... */`) are out of scope regardless.
    /// Defaults to `false`.
    include_plain_comments: bool,
    /// Replacement form used inside plain `//` comments when
    /// `include_plain_comments = true`. Defaults to `bare_url`.
    /// Ignored for doc comments and when no repository is
    /// configured.
    plain_comment_form: PlainForm,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            forge: None,
            repository: None,
            suggest_issue_url: true,
            suggest_pr_url: true,
            doc_comment_form: DocForm::Inline,
            include_plain_comments: false,
            plain_comment_form: PlainForm::BareUrl,
        }
    }
}

pub struct BareIssueReference {
    forge: Option<Forge>,
    /// The repository's `repository` URL normalised to the canonical
    /// web base `https://host/owner/repo` (see [`repo_url::normalize`]),
    /// or `None` when unset or unparseable.
    repo_web_base: Option<String>,
    suggest_issue_url: bool,
    suggest_pr_url: bool,
    doc_comment_form: DocForm,
    include_plain_comments: bool,
    plain_comment_form: PlainForm,
}

/// Resolve the `repository` config value to its canonical web base.
/// `None` when unset — the lint then runs help-only. **Panics** when
/// it is set but unparseable, so a malformed URL fails loudly at
/// launch rather than silently disabling the autofix and leaving the
/// author to wonder why no link is suggested.
fn resolve_repository(repository: Option<&str>) -> Option<String> {
    repository.map(|raw| {
        repo_url::normalize(raw).unwrap_or_else(|| {
            panic!(
                "perfectionist::bare_issue_reference: `repository = {raw:?}` is not a \
                 parseable git URL; expected an HTTP(S) URL, an `ssh://` URL, or the \
                 scp-like `git@host:owner/repo.git` form, with an `owner/repo` path",
            )
        })
    })
}

impl BareIssueReference {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            forge: config.forge,
            repo_web_base: resolve_repository(config.repository.as_deref()),
            suggest_issue_url: config.suggest_issue_url,
            suggest_pr_url: config.suggest_pr_url,
            doc_comment_form: config.doc_comment_form,
            include_plain_comments: config.include_plain_comments,
            plain_comment_form: config.plain_comment_form,
        }
    }

    /// The forge whose path layout to use: the explicit `forge` if
    /// set, otherwise one detected from the repository's host. `None`
    /// when neither yields a recognised forge — there is no fallback,
    /// so the rule never prefers one service.
    fn effective_forge(&self) -> Option<Forge> {
        self.forge
            .or_else(|| self.repo_web_base.as_deref().and_then(Forge::detect))
    }

    /// Suggested issue URL for `number`. `None` (so: help-only)
    /// unless `repository` is set — it carries the owner/repo path
    /// the link needs — and a forge is known (explicit or detected
    /// from the host).
    fn issue_url(&self, number: &str) -> Option<String> {
        let base = self.repo_web_base.as_deref()?;
        let forge = self.effective_forge()?;
        Some(join_url(base, forge.issue_path(), number))
    }

    /// Suggested pull-request URL for `number`; see [`Self::issue_url`]
    /// for when it is `None`.
    fn pr_url(&self, number: &str) -> Option<String> {
        let base = self.repo_web_base.as_deref()?;
        let forge = self.effective_forge()?;
        Some(join_url(base, forge.pr_path(), number))
    }

    /// Whether a bare `#NNN` reference can denote a pull request on
    /// the effective forge. False on GitLab, where `#NNN` is always an
    /// issue (merge requests are written `!NNN`); true otherwise.
    fn hash_can_mean_pr(&self) -> bool {
        self.effective_forge() != Some(Forge::GitLab)
    }
}

/// Join a repository base URL with a relative path (the part after
/// the base), substituting `{number}`. Exactly one `/` separates the
/// base from the path regardless of a trailing slash on the base or a
/// leading slash on the path.
fn join_url(base_url: &str, path: &str, number: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let path = path.replace("{number}", number);
    format!("{base}/{}", path.trim_start_matches('/'))
}

/// A recognised git-hosting service. The chosen forge fixes the
/// issue / PR URL layout. It can be given explicitly (needed for a
/// self-hosted instance, whose host isn't recognised) or left unset
/// and detected from the `repository` host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Forge {
    /// GitHub or a GitHub Enterprise instance. Paths:
    /// `/issues/{number}`, `/pull/{number}`.
    GitHub,
    /// GitLab (gitlab.com or self-hosted). Paths:
    /// `/-/issues/{number}`, `/-/merge_requests/{number}`.
    GitLab,
    /// Gitea / Forgejo (including Codeberg). Paths:
    /// `/issues/{number}`, `/pulls/{number}`.
    Gitea,
}

impl Forge {
    fn issue_path(self) -> &'static str {
        match self {
            Forge::GitHub | Forge::Gitea => "/issues/{number}",
            Forge::GitLab => "/-/issues/{number}",
        }
    }

    fn pr_path(self) -> &'static str {
        match self {
            Forge::GitHub => "/pull/{number}",
            Forge::Gitea => "/pulls/{number}",
            Forge::GitLab => "/-/merge_requests/{number}",
        }
    }

    /// Detect the forge from a repository URL's host — both the
    /// public instances and the conventional `gitlab.*` / `github.*`
    /// / `gitea.*` / `forgejo.*` self-hosted subdomains. Returns
    /// `None` for a host that gives no such hint, so it needs an
    /// explicit `forge`.
    fn detect(repo_url: &str) -> Option<Forge> {
        let host = host_of(repo_url)?.to_ascii_lowercase();
        // Public instances, matched on the full host.
        match host.as_str() {
            "github.com" => return Some(Forge::GitHub),
            "gitlab.com" => return Some(Forge::GitLab),
            // Forgejo (Codeberg) shares Gitea's URL layout.
            "codeberg.org" | "gitea.com" => return Some(Forge::Gitea),
            _ => {}
        }
        // Self-hosted instances conventionally live under a subdomain
        // named after the software (`gitlab.example.com`,
        // `gitea.example.com`, `github.example.com` for GitHub
        // Enterprise), so the leading DNS label names the forge.
        // `git.*` is the most common generic prefix but ambiguous
        // (Gitea, GitLab, cgit, gitweb), so it isn't matched.
        //
        // Two whole services are deliberately unsupported, because a
        // bare `#NNN` doesn't reliably map to one of their URLs:
        // Gitee (`gitee.com`) numbers issues with alphanumeric tokens
        // rather than the integers `#NNN` assumes, and Bitbucket's
        // `#NNN` convention (issue? PR? neither?) we couldn't pin
        // down, so we don't guess at its links.
        match host.split('.').next()? {
            "gitlab" => Some(Forge::GitLab),
            "github" => Some(Forge::GitHub),
            "gitea" | "forgejo" => Some(Forge::Gitea),
            _ => None,
        }
    }
}

/// Host component of a URL like `https://github.com/owner/repo` →
/// `"github.com"`. `None` if the URL has no `://`.
fn host_of(url: &str) -> Option<&str> {
    let after_scheme = url.find("://")?;
    let rest = &url[after_scheme + 3..];
    let end = rest.find(['/', '?', '#', ':']).unwrap_or(rest.len());
    Some(&rest[..end])
}

impl_lint_pass!(BareIssueReference => [BARE_ISSUE_REFERENCE]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[BARE_ISSUE_REFERENCE]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("bare_issue_reference", DefaultState::Active) {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(BareIssueReference::new()));
}

/// One bare `#NNN` finding, parked during the comment walk and emitted
/// later at its enclosing HIR node. Everything the diagnostic needs
/// that depends on the comment chunk (the link URLs, the
/// `reference`-form definition site) is precomputed here, since the
/// chunk is gone by the time the late pass emits.
struct IssueRefViolation {
    token: String,
    issue_url: Option<String>,
    pr_url: Option<String>,
    is_doc: bool,
    ref_site: Option<RefDefinitionSite>,
}

/// The crate-uniform rendering choices for the bare `#NNN` autofix,
/// resolved once and shared by every emitted finding.
#[derive(Clone, Copy)]
struct EmitOptions {
    suggest_issue: bool,
    suggest_pr: bool,
    /// Whether `suggest_pr_url` is capable of producing a suggestion
    /// on the effective forge at all — see
    /// [`BareIssueReference::hash_can_mean_pr`]. Distinct from
    /// `suggest_pr`, which also folds in the knob's own value; the
    /// "no knob enabled" help needs the two apart so it doesn't
    /// advise turning on a knob that cannot help.
    hash_can_mean_pr: bool,
    doc_form: DocForm,
    plain_form: PlainForm,
}

impl<'tcx> LateLintPass<'tcx> for BareIssueReference {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let mut violations: Vec<(Span, IssueRefViolation)> = Vec::new();
        walk_local_comments(lint_context, |chunk| match chunk.surface {
            CommentSurface::DocBlock | CommentSurface::DocBlockBlock => {
                self.scan_doc(chunk, &mut violations);
            }
            CommentSurface::PlainLine => {
                if self.include_plain_comments {
                    self.scan_plain(chunk, &mut violations);
                }
            }
            CommentSurface::PlainBlock => {
                // Out of scope per the plan — `bare_issue_reference`
                // deliberately doesn't scan plain block comments.
            }
        });
        let options = EmitOptions {
            suggest_issue: self.suggest_issue_url,
            // On GitLab a bare `#NNN` always denotes an issue — merge
            // requests are written `!NNN` — so the PR suggestion never
            // applies to it, whatever `suggest_pr_url` says.
            suggest_pr: self.suggest_pr_url && self.hash_can_mean_pr(),
            hash_can_mean_pr: self.hash_can_mean_pr(),
            doc_form: self.doc_comment_form,
            plain_form: self.plain_comment_form,
        };
        emit_at_enclosing_hir(lint_context.tcx, violations, |hir_id, span, violation| {
            emit_issue_ref(lint_context, hir_id, span, violation, options);
        });
    }
}

impl BareIssueReference {
    fn scan_doc(&self, chunk: &CommentChunk<'_>, out: &mut Vec<(Span, IssueRefViolation)>) {
        let skips = scan_skip_regions(&chunk.rendered);
        self.scan(chunk, &skips, true, out);
    }

    fn scan_plain(&self, chunk: &CommentChunk<'_>, out: &mut Vec<(Span, IssueRefViolation)>) {
        self.scan(chunk, &[], false, out);
    }

    fn scan(
        &self,
        chunk: &CommentChunk<'_>,
        skips: &[core::ops::Range<usize>],
        is_doc: bool,
        out: &mut Vec<(Span, IssueRefViolation)>,
    ) {
        let text = &chunk.rendered;
        let bytes = text.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != b'#' {
                index += utf8_char_len(bytes, index);
                continue;
            }
            // Left-context guard: skip if preceded by a word
            // character (would make the `#NNN` part of a larger
            // identifier), `[` (existing markdown link label), or
            // `` ` `` (markdown code span / plain-comment
            // backtick-quoted code).
            if index > 0 {
                let prev = bytes[index - 1];
                if prev.is_ascii_alphanumeric() || prev == b'_' || prev == b'[' || prev == b'`' {
                    index += 1;
                    continue;
                }
            }
            // Take digits after `#`.
            let digits_start = index + 1;
            let mut end = digits_start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end == digits_start {
                index += 1;
                continue;
            }
            // Right-context: must end at word boundary.
            if end < bytes.len() {
                let next = bytes[end];
                if next.is_ascii_alphanumeric() || next == b'_' {
                    index = end;
                    continue;
                }
            }
            // URL-fragment guard.
            if back_scan_url_fragment(text, index) {
                index = end;
                continue;
            }
            if position_in_skip(skips, index) {
                index = end;
                continue;
            }
            let number = &text[digits_start..end];
            self.collect(chunk, index, end - index, number, is_doc, out);
            index = end;
        }
    }

    fn collect(
        &self,
        chunk: &CommentChunk<'_>,
        rendered_pos: usize,
        len: usize,
        number: &str,
        is_doc: bool,
        out: &mut Vec<(Span, IssueRefViolation)>,
    ) {
        let Some(span) = chunk.span_for(rendered_pos, len as u32) else {
            return;
        };
        // For the `reference` form, work out where (and with what
        // prefix) to append the `[#N]: URL` definition, so the fix is
        // complete rather than leaving the definition to the author.
        // `None` for other forms, block doc comments, or a block that
        // already defines the reference.
        let ref_site = (self.doc_comment_form == DocForm::Reference)
            .then(|| reference_definition_site(chunk, rendered_pos, number))
            .flatten();
        out.push((
            span,
            IssueRefViolation {
                token: format!("#{number}"),
                issue_url: self.issue_url(number),
                pr_url: self.pr_url(number),
                is_doc,
                ref_site,
            },
        ));
    }
}

/// Emit one bare `#NNN` finding at its enclosing HIR node. The
/// suggestion shape is driven by `doc_form` / `plain_form`; the issue /
/// PR link URLs were resolved when the violation was parked.
fn emit_issue_ref(
    lint_context: &LateContext<'_>,
    hir_id: HirId,
    span: Span,
    violation: IssueRefViolation,
    options: EmitOptions,
) {
    let IssueRefViolation {
        token,
        issue_url,
        pr_url,
        is_doc,
        ref_site,
    } = violation;
    let EmitOptions {
        suggest_issue,
        suggest_pr,
        hash_can_mean_pr,
        doc_form,
        plain_form,
    } = options;
    span_lint_hir_and_then(
        lint_context,
        BARE_ISSUE_REFERENCE,
        hir_id,
        span,
        format!(
            "ambiguous `{token}`; a bare `#NNN` could be an issue or pull request, \
             a colour, or any other numbered item",
        ),
        move |diag| {
            // Offer the issue / PR link suggestions when a
            // repository is configured and at least one knob is
            // on. These are only ever `MaybeIncorrect`: the token
            // is ambiguous, so the lint can't be sure it even
            // names a reference.
            match issue_url {
                None => {
                    diag.help(
                        "set `repository` (and `forge`, if its host isn't a recognised \
                         service) in dylint.toml under \
                         `[\"perfectionist::bare_issue_reference\"]` to enable issue / PR \
                         link suggestions",
                    );
                }
                Some(_) if !(suggest_issue || suggest_pr) => {
                    // Name only the knobs that can actually produce a
                    // suggestion here. On GitLab `suggest_pr_url` is
                    // inert whatever its value, so listing it would
                    // send the author after a setting that changes
                    // nothing — and, when it is already `true`, would
                    // repeat this very diagnostic verbatim.
                    diag.help(if hash_can_mean_pr {
                        "enable `suggest_issue_url` and/or `suggest_pr_url` in dylint.toml \
                         under `[\"perfectionist::bare_issue_reference\"]` to get a link \
                         suggestion"
                    } else {
                        "enable `suggest_issue_url` in dylint.toml under \
                         `[\"perfectionist::bare_issue_reference\"]` to get a link suggestion; \
                         `suggest_pr_url` has no effect on GitLab, where a bare `#NNN` is \
                         always an issue (merge requests are written `!NNN`)"
                    });
                }
                Some(issue_url) => {
                    if suggest_issue {
                        emit_one(
                            diag,
                            span,
                            &token,
                            &issue_url,
                            "issue",
                            is_doc,
                            doc_form,
                            plain_form,
                            ref_site.as_ref(),
                        );
                    }
                    if suggest_pr {
                        let pr_url = pr_url.expect("pr_url renders whenever issue_url does");
                        emit_one(
                            diag,
                            span,
                            &token,
                            &pr_url,
                            "pull request",
                            is_doc,
                            doc_form,
                            plain_form,
                            ref_site.as_ref(),
                        );
                    }
                }
            }
            // Two disambiguations that apply whatever the config:
            // mark the token as not-a-reference, or avoid the
            // `#NNN` spelling entirely.
            diag.help(format!(
                "if `{token}` is not an issue or pull request, enclose it in backticks \
                 so it renders as code",
            ));
            diag.help("or refer to the numbered item with a spelling that has no leading `#`");
        },
    );
}

/// Where to append a `reference`-form `[#N]: URL` definition, so the
/// fix is complete instead of leaving the definition to the author.
struct RefDefinitionSite {
    /// Zero-width span at the end of the doc block.
    block_end: Span,
    /// The `///` / `//!` prefix (with indentation) the appended
    /// definition lines start with, so they stay part of the block.
    doc_prefix: String,
}

/// Resolve the `reference`-form definition site for a `#NNN` at
/// `rendered_pos`, or `None` when the definition shouldn't be
/// appended: a `/** */` block doc comment (the definition would need
/// to go before `*/`, which this doesn't handle), or a block that
/// already defines the reference.
fn reference_definition_site(
    chunk: &CommentChunk<'_>,
    rendered_pos: usize,
    number: &str,
) -> Option<RefDefinitionSite> {
    if chunk.surface != CommentSurface::DocBlock {
        return None;
    }
    if block_defines_reference(&chunk.rendered, number) {
        return None;
    }
    Some(RefDefinitionSite {
        block_end: chunk.source_span.shrink_to_hi(),
        doc_prefix: doc_line_prefix(chunk, rendered_pos)?,
    })
}

/// Whether the rendered doc block already carries a `[#NNN]: ...`
/// reference-link definition, so the completed fix shouldn't append a
/// duplicate.
fn block_defines_reference(rendered: &str, number: &str) -> bool {
    let label = format!("[#{number}]:");
    rendered
        .lines()
        .any(|line| line.trim_start().starts_with(&label))
}

/// The `///` / `//!` prefix (with leading indentation, the optional
/// content space trimmed) of the source line holding `rendered_pos`.
/// The appended definition lines reuse it so they remain part of the
/// same doc block at the same indentation.
fn doc_line_prefix(chunk: &CommentChunk<'_>, rendered_pos: usize) -> Option<String> {
    let line = chunk.lines.iter().find(|line| {
        rendered_pos >= line.rendered_start
            && rendered_pos < line.rendered_start + line.rendered_len
    })?;
    let src = chunk.source_file.src.as_deref()?;
    let content_offset = line.source_offset as usize;
    let line_start = src
        .get(..content_offset)?
        .rfind('\n')
        .map_or(0, |nl| nl + 1);
    let prefix = src.get(line_start..content_offset)?;
    Some(prefix.trim_end_matches(' ').to_owned())
}

/// Emit one issue-or-PR link suggestion. `target_label` is `"issue"`
/// or `"pull request"`. Every suggestion is `MaybeIncorrect`: a bare
/// `#NNN` is ambiguous, so the lint can never be confident the token
/// names a reference at all, let alone which kind. When `ref_site` is
/// set (the `reference` form on a line doc block), the suggestion is a
/// multipart edit that also appends the `[#N]: URL` definition.
#[expect(
    clippy::too_many_arguments,
    reason = "a small private emit helper; bundling these into a struct would obscure the call"
)]
fn emit_one(
    diag: &mut rustc_errors::Diag<'_, ()>,
    span: Span,
    token: &str,
    url: &str,
    target_label: &str,
    is_doc: bool,
    doc_form: DocForm,
    plain_form: PlainForm,
    ref_site: Option<&RefDefinitionSite>,
) {
    if is_doc {
        if let (DocForm::Reference, Some(site)) = (doc_form, ref_site) {
            // A blank doc line precedes the definition so CommonMark
            // doesn't fold `[#N]: URL` into the preceding paragraph
            // as lazy continuation (a definition can't interrupt a
            // paragraph).
            let definition = format!(
                "\n{prefix}\n{prefix} [{token}]: {url}",
                prefix = site.doc_prefix,
            );
            diag.multipart_suggestion(
                format!(
                    "use a reference-style markdown link to the {target_label} \
                     (its `[#N]: URL` definition is appended to the doc block)",
                ),
                vec![
                    (span, render_doc_suggestion(DocForm::Reference, token, url)),
                    (site.block_end, definition),
                ],
                Applicability::MaybeIncorrect,
            );
            return;
        }
        let message = match doc_form {
            DocForm::Inline => format!("use an inline markdown link to the {target_label}"),
            DocForm::Reference => format!(
                "use a reference-style markdown link to the {target_label} \
                 (ensure a `[#N]: URL` definition exists in the doc block)",
            ),
            DocForm::BareUrl => format!("substitute the {target_label} URL"),
            DocForm::BracketedUrl => {
                format!("substitute the {target_label} URL wrapped in `<...>`")
            }
        };
        diag.span_suggestion(
            span,
            message,
            render_doc_suggestion(doc_form, token, url),
            Applicability::MaybeIncorrect,
        );
    } else {
        let message = match plain_form {
            PlainForm::BareUrl => format!("substitute the {target_label} URL"),
            PlainForm::BracketedUrl => {
                format!("substitute the {target_label} URL wrapped in `<...>`")
            }
        };
        diag.span_suggestion(
            span,
            message,
            render_plain_suggestion(plain_form, url),
            Applicability::MaybeIncorrect,
        );
    }
}

fn render_doc_suggestion(form: DocForm, token: &str, url: &str) -> String {
    match form {
        DocForm::Inline => format!("[{token}]({url})"),
        // For the reference form the matched span becomes `[#N]`; the
        // matching `[#N]: url` definition is appended separately by
        // `emit_one` (line doc blocks) or left to the author (`/** */`
        // block doc comments). Same shape as rustdoc's
        // collapsed-reference form.
        DocForm::Reference => format!("[{token}]"),
        // The URL forms replace the `#N` token outright; the `token`
        // is intentionally dropped.
        DocForm::BareUrl => url.to_owned(),
        DocForm::BracketedUrl => format!("<{url}>"),
    }
}

fn render_plain_suggestion(form: PlainForm, url: &str) -> String {
    match form {
        PlainForm::BareUrl => url.to_owned(),
        PlainForm::BracketedUrl => format!("<{url}>"),
    }
}

#[cfg(test)]
mod tests;
