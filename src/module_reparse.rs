//! Shared machinery for the import-rewriting rules that must reach
//! out-of-line `mod foo;` submodules.
//!
//! `import_granularity` and `self_import` both rewrite `use` statements
//! and both need to see every `use` in the crate, including those in
//! separate-file submodules. A pre-expansion `EarlyLintPass` cannot: an
//! out-of-line `mod foo;` is still `ModKind::Unloaded` until macro
//! expansion, so the pass only ever sees the crate-root file and inline
//! `mod { ... }` blocks. A post-expansion AST has every module loaded but
//! has also stripped `#[cfg(...)]`-gated code, which both rules want to
//! keep linting.
//!
//! [`for_each_module_file`] threads the needle: from a late pass it
//! re-parses each of the crate's module source files from a throwaway
//! [`ParseSess`] that shares the real [`SourceMap`] (so spans — and the
//! suggestions built from them — point at the real files) but routes
//! parse errors to a [`SilentEmitter`]. Re-parsing reaches every
//! module-scoped submodule (the crate root and every `mod` declared at
//! module scope, at any depth) while keeping `#[cfg(...)]` gates intact,
//! because parsing does not evaluate cfg.

use std::collections::HashSet;
use std::sync::Arc;

use rustc_ast::Crate;
use rustc_errors::DiagCtxt;
use rustc_errors::emitter::SilentEmitter;
use rustc_lint::{LateContext, LintContext};
use rustc_parse::lexer::StripTokens;
use rustc_parse::new_parser_from_source_str;
use rustc_session::parse::ParseSess;
use rustc_span::def_id::LOCAL_CRATE;
use rustc_span::source_map::SourceMap;
use rustc_span::{FileName, SourceFile};

/// Re-parse every on-disk source file that backs a module in the crate's
/// HIR module tree, calling `handle` once with each successfully-parsed
/// module as a standalone [`Crate`]. Within a single file, only that
/// file's own items are present — an out-of-line `mod foo;` it declares
/// is `ModKind::Unloaded` in a fresh parse, but `foo`'s file appears in
/// the source map in its own right and is handled by its own `handle`
/// call, so a caller's walk stays within one file at a time.
///
/// Re-parsing is scoped to files that define a module in the HIR module
/// tree, so source-map files that are *not* standalone modules —
/// `include!` fragments, `include_str!`-ed data, proc-macro-synthesised
/// modules — are never re-parsed and wrongly flagged.
///
/// The module tree is enumerated from the crate root and the crate's
/// *free* items, so a `mod` declared at module scope (nested to any
/// depth) is covered, but an out-of-line module declared inside a
/// function body — a `#[path]`-only construct, since a body `mod foo;`
/// does not otherwise resolve to a file — is not.
pub(crate) fn for_each_module_file(cx: &LateContext<'_>, mut handle: impl FnMut(&Crate)) {
    let tcx = cx.tcx;
    let source_map = cx.sess().psess.clone_source_map();

    // The files that define a module in this crate's module tree.
    let mut module_files: HashSet<FileName> = HashSet::new();
    record_module_file(&source_map, &mut module_files, tcx.hir_root_module().spans);
    for item_id in tcx.hir_free_items() {
        if let rustc_hir::ItemKind::Mod(_, module) = &tcx.hir_item(item_id).kind {
            record_module_file(&source_map, &mut module_files, module.spans);
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

    // A throwaway `ParseSess` sharing the real `SourceMap` (so spans, and
    // suggestions, point at the real files) but with a silenced
    // `DiagCtxt`, so a file that does not parse cleanly standalone is
    // skipped rather than surfacing parse errors.
    let mut parse_psess = ParseSess::with_dcx(
        DiagCtxt::new(Box::new(SilentEmitter)),
        Arc::clone(&source_map),
    );
    // `with_dcx` already derives this from the root expansion (the crate's
    // edition), but set it explicitly so edition-sensitive syntax
    // re-parses exactly as the crate compiles.
    parse_psess.edition = cx.sess().edition();

    for source_file in &module_source_files {
        if let Some(krate) = parse_module_file(&parse_psess, source_file) {
            handle(&krate);
        }
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
/// deduplicates by name, so the parser reuses the loaded `SourceFile`
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
