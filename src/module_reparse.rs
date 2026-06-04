//! Re-parsing a crate's module files from a `LateLintPass`.
//!
//! A rule that inspects the *source-level* layout of `use` statements —
//! the blank-line grouping of `perfectionist::import_grouping`, the
//! granularity of `perfectionist::import_granularity`, the `self`
//! handling of `perfectionist::self_import` — hits a wall in a
//! pre-expansion `EarlyLintPass`: an out-of-line `mod foo;` module is
//! still `ModKind::Unloaded` there (its file is not parsed until macro
//! expansion), so the walk never sees it and silently skips every
//! separate-file submodule.
//!
//! Running in a `LateLintPass` and re-parsing each module file instead
//! reaches every submodule while keeping `#[cfg(...)]` gates intact —
//! parsing does not strip cfg, unlike the post-expansion AST, which is
//! why the pre-expansion pass existed in the first place.
//!
//! Two entry points share the same re-parse machinery:
//! [`parse_crate_module_files`] returns every file's freshly parsed
//! [`Crate`] alongside the body spans of the crate's live modules (the
//! inline-recursion guard a caller needs to skip cfg-disabled inline
//! modules), and [`for_each_module_file`] is a thin callback wrapper for
//! callers that handle one file at a time and do their own descent.

use rustc_ast::Crate;
use rustc_errors::DiagCtxt;
use rustc_errors::emitter::SilentEmitter;
use rustc_lint::{LateContext, LintContext};
use rustc_parse::lexer::StripTokens;
use rustc_parse::new_parser_from_source_str;
use rustc_session::parse::ParseSess;
use rustc_span::def_id::LOCAL_CRATE;
use rustc_span::source_map::SourceMap;
use rustc_span::{BytePos, FileName, SourceFile};
use std::collections::HashSet;
use std::sync::Arc;

/// The byte range `(lo, hi)` of a module body, used as a stable key
/// across the re-parse / HIR boundary. Full [`rustc_span::Span`]
/// equality also compares the `parent` `LocalDefId` and `SyntaxContext`,
/// which differ between a freshly re-parsed span and the HIR-lowered
/// span of the same source bytes; the byte range matches and uniquely
/// identifies a module body.
pub(crate) type SpanRange = (BytePos, BytePos);

/// Re-parse every on-disk source file that backs a module in this
/// crate's HIR module tree, returning each file's freshly parsed
/// [`Crate`] (crate root plus every out-of-line `mod foo;` file) along
/// with `live_module_spans`.
///
/// `live_module_spans` is the body [`SpanRange`] of every module live in
/// the compiled crate (each inline `mod m { ... }` and out-of-line
/// `mod foo;`). A re-parse keeps cfg-disabled modules — parsing does not
/// strip cfg — so a caller walking the re-parsed AST must consult this
/// set before descending into an inline module, or it would lint a
/// `#[cfg(FALSE)] mod m { ... }` (e.g. `#[cfg(test)] mod tests`) that is
/// not part of the build. The result is a tuple rather than a named
/// struct because a struct field of type [`Vec<Crate>`] makes rustdoc's
/// auto-trait synthesis overflow on the AST's recursive type graph.
///
/// The throwaway [`ParseSess`] shares the real [`SourceMap`], so every
/// span in the returned ASTs — and any suggestion built from them —
/// points at the real files. Its [`DiagCtxt`] is wired to a
/// [`SilentEmitter`], so a file that does not parse as a standalone
/// module is skipped rather than surfacing parse errors.
///
/// Re-parsing is scoped to files that actually back a module in the HIR
/// module tree, which excludes `include!` fragments, `include_str!`-ed
/// `.rs` data, and proc-macro-synthesised modules — none of which should
/// be re-parsed and flagged as if the user wrote them as a module.
pub(crate) fn parse_crate_module_files(
    lint_context: &LateContext<'_>,
) -> (Vec<Crate>, HashSet<SpanRange>) {
    let tcx = lint_context.tcx;
    let source_map = lint_context.sess().psess.clone_source_map();

    // The files that define a module in this crate's module tree.
    let module_files = crate_module_files(lint_context);

    // The body span of every live module (for the inline-recursion guard
    // documented on this function's returned `live_module_spans`).
    let mut live_module_spans: HashSet<SpanRange> = HashSet::new();
    for item_id in tcx.hir_free_items() {
        if let rustc_hir::ItemKind::Mod(_, module) = &tcx.hir_item(item_id).kind {
            let inner = module.spans.inner_span;
            live_module_spans.insert((inner.lo(), inner.hi()));
        }
    }

    // Snapshot the files before parsing: re-parsing takes a write lock on
    // the shared source map, so it must not run while the `files()` read
    // guard is held.
    let module_source_files: Vec<Arc<SourceFile>> = {
        let source_files = source_map.files();
        source_files
            .iter()
            .filter(|source_file| source_file.cnum == LOCAL_CRATE)
            .filter(|source_file| module_files.contains(&source_file.name))
            .cloned()
            .collect()
    };

    let mut parse_psess = ParseSess::with_dcx(
        DiagCtxt::new(Box::new(SilentEmitter)),
        Arc::clone(&source_map),
    );
    // `with_dcx` already derives this from the root expansion (the
    // crate's edition), but set it explicitly so edition-sensitive syntax
    // re-parses exactly as the crate compiles.
    parse_psess.edition = lint_context.sess().edition();

    let crates = module_source_files
        .iter()
        .filter_map(|source_file| parse_module_file(&parse_psess, source_file))
        .collect();

    (crates, live_module_spans)
}

/// The on-disk source files that back a module in this crate's HIR
/// module tree, keyed by [`FileName`].
///
/// This is the set of files the user actually wrote as Rust modules —
/// the crate root and every out-of-line `mod foo;` file. It deliberately
/// excludes everything that lands in the source map without being a
/// module the user authored: `include_str!` / `include_bytes!` data
/// files (which may be YAML, lock files, plain text, ...), `include!`
/// fragments spliced inline rather than backing their own module, and
/// proc-macro-synthesised `<proc-macro source>` modules.
///
/// The comment-scanning rules tokenize the local crate's files as Rust
/// and must filter the source map through this set: `bare_url`,
/// `bare_email`, `bare_issue_reference`, and `unicode_ellipsis_in_docs`
/// via the shared [`crate::comment_walk::walk_local_comments`] walker, plus
/// `unicode_ellipsis_in_comments` through its own token loop. Otherwise
/// a bare `http(s)://` URL inside an
/// `include_str!`-ed YAML file lexes as a `//` line comment and gets
/// flagged (and, worse, autofix-rewritten) as if it were a Rust comment.
/// See <https://github.com/KSXGitHub/perfectionist/issues/179>.
pub(crate) fn crate_module_files(lint_context: &LateContext<'_>) -> HashSet<FileName> {
    let tcx = lint_context.tcx;
    let source_map = lint_context.sess().source_map();
    let mut module_files: HashSet<FileName> = HashSet::new();
    record_module_file(source_map, &mut module_files, tcx.hir_root_module().spans);
    for item_id in tcx.hir_free_items() {
        if let rustc_hir::ItemKind::Mod(_, module) = &tcx.hir_item(item_id).kind {
            record_module_file(source_map, &mut module_files, module.spans);
        }
    }
    module_files
}

/// Re-parse every on-disk source file that backs a module in the crate's
/// HIR module tree, calling `handle` once with each successfully-parsed
/// module as a standalone [`Crate`]. Within a single file, only that
/// file's own items are present — an out-of-line `mod foo;` it declares
/// is `ModKind::Unloaded` in a fresh parse, but `foo`'s file appears in
/// the source map in its own right and is handled by its own `handle`
/// call, so a caller's walk stays within one file at a time.
///
/// This is a thin wrapper over [`parse_crate_module_files`] for callers
/// that process one file at a time and do not need the
/// `live_module_spans` inline-recursion guard. A caller that descends
/// into inline `mod { ... }` bodies itself is responsible for whatever
/// cfg handling it needs.
///
/// The module tree is enumerated from the crate root and the crate's
/// *free* items, so a `mod` declared at module scope (nested to any
/// depth) is covered, but an out-of-line module declared inside a
/// function body — a `#[path]`-only construct, since a body `mod foo;`
/// does not otherwise resolve to a file — is not.
pub(crate) fn for_each_module_file(cx: &LateContext<'_>, mut handle: impl FnMut(&Crate)) {
    let (crates, _live_module_spans) = parse_crate_module_files(cx);
    for krate in &crates {
        handle(krate);
    }
}

/// Record the on-disk source file that holds a module's body, keyed by
/// name. A dummy span (no real body) contributes nothing. Only
/// [`FileName::Real`] files count: a module synthesised by a proc macro
/// has a `<proc-macro source>` file that must not be re-parsed and
/// flagged as if the user wrote it.
fn record_module_file(
    source_map: &SourceMap,
    module_files: &mut HashSet<FileName>,
    spans: rustc_hir::ModSpans,
) {
    let inner_span = spans.inner_span;
    if inner_span.is_dummy() {
        return;
    }
    let name = &source_map.lookup_source_file(inner_span.lo()).name;
    if matches!(name, FileName::Real(_)) {
        module_files.insert(name.clone());
    }
}

/// Re-parse a module's source file from its already-loaded text. Returns
/// `None` (silently discarding buffered diagnostics — `parse_psess` is
/// wired to a [`SilentEmitter`]) when the file does not parse as a
/// standalone module. The shared source map already holds this file and
/// deduplicates by name, so the parser reuses the loaded [`SourceFile`]
/// (preserving the real spans) and the passed source text is ignored —
/// hence the empty string, which avoids both a disk re-read and a clone
/// of the whole file.
fn parse_module_file(parse_psess: &ParseSess, source_file: &SourceFile) -> Option<Crate> {
    // Load-bearing: a `SourceFile` without in-memory source makes the
    // lexer ICE ("cannot lex `source_file` without source"). Local-crate
    // `Real` files normally carry it, but bail rather than risk the ICE.
    source_file.src.as_ref()?;
    let mut parser = match new_parser_from_source_str(
        parse_psess,
        source_file.name.clone(),
        String::new(),
        StripTokens::ShebangAndFrontmatter,
    ) {
        Ok(parser) => parser,
        Err(errors) => {
            for error in errors {
                error.cancel();
            }
            return None;
        }
    };
    match parser.parse_crate_mod() {
        Ok(krate) => Some(krate),
        Err(error) => {
            error.cancel();
            None
        }
    }
}
