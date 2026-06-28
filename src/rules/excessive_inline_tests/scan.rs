//! The per-crate HIR walk. Classifies every item as production or
//! test, accumulates each source file's inline-test footprint, and
//! emits the inline-style diagnostics once per file.

use super::cfg_test::cfg_predicate_implies_test;
use super::config::{ExcessiveInlineTests, InlineStyle};
use super::{EXCESSIVE_INLINE_TESTS, paths};
use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::is_test_function;
use rustc_hir::{Item, ItemKind, Mod};
use rustc_lint::{LateContext, LintContext};
use rustc_span::hygiene::ExpnKind;
use rustc_span::source_map::SourceMap;
use rustc_span::{BytePos, SourceFile, Span, Symbol};
use std::collections::HashMap;
use std::sync::Arc;

/// One inline test item charged to a file's footprint.
struct TestItem {
    span: Span,
    /// The identifier of an inline `#[cfg(test)] mod <name> { ... }`
    /// block, or `None` for a bare top-level test item (a `#[test] fn`,
    /// `#[cfg(test)] fn`, etc.) that has no module name. Used to name
    /// the extraction target in the help text.
    module_name: Option<Symbol>,
}

/// Per-source-file tally built up during the walk.
#[derive(Default)]
struct FileAcc {
    /// Top-level production items seen in this file. A file with zero
    /// of these is entirely test code and exempt from the inline-style
    /// check — it is itself a valid extraction target.
    production_count: usize,
    /// The inline test items contributing to the footprint.
    test_items: Vec<TestItem>,
    /// Sum of the line spans of every item in `test_items`.
    inline_test_lines: usize,
    /// The file these tallies belong to, kept for line counting and its
    /// path. `None` only before the first item is recorded.
    file: Option<Arc<SourceFile>>,
}

pub(super) fn run(state: &ExcessiveInlineTests, cx: &LateContext<'_>) {
    // Integration tests, benchmarks, and examples are separate crates
    // Cargo hands the rule under `--all-targets`; their test code is the
    // target itself, not misplaced unit tests, so leave them untouched.
    if paths::is_separate_test_target(cx) {
        return;
    }
    let mut files: HashMap<BytePos, FileAcc> = HashMap::new();
    walk(cx, cx.tcx.hir_root_module(), &mut files);
    for acc in files.values() {
        emit_inline_style(state, cx, acc);
    }
}

fn walk(cx: &LateContext<'_>, module: &Mod<'_>, files: &mut HashMap<BytePos, FileAcc>) {
    for item_id in module.item_ids {
        classify(cx, cx.tcx.hir_item(*item_id), files);
    }
}

fn classify(cx: &LateContext<'_>, item: &Item<'_>, files: &mut HashMap<BytePos, FileAcc>) {
    // The rule runs in a `cfg(test)` build, so the AST it sees contains
    // items that are not the author's hand-written layout. Treat them by
    // origin rather than skipping every expansion wholesale:
    let expn_kind = item.span.ctxt().outer_expn_data().kind;
    if matches!(expn_kind, ExpnKind::AstPass(_) | ExpnKind::Desugaring(_)) {
        // Compiler-synthesised: the test harness `main`, `extern crate
        // test`, the descriptor const, AST desugarings. Not the
        // author's code and tied to no file they control — ignore them
        // entirely so they neither charge the footprint nor inflate a
        // file's production count (which would rob an all-test crate
        // root of its exemption).
        return;
    }

    let cfg_test = cfg_predicate_implies_test(cx.tcx, item.hir_id());
    let is_test = cfg_test
        || (matches!(item.kind, ItemKind::Fn { .. })
            && is_test_function(cx.tcx, item.owner_id.def_id));

    if matches!(expn_kind, ExpnKind::Macro(..)) {
        // A user macro invocation is itself a hand-written item at its
        // call site. Count a production expansion toward that file so a
        // file whose only production is macro-generated still fails the
        // all-test exemption; but never charge macro-expanded test code
        // to the budget (a single macro call must not bust it), and do
        // not recurse into the expansion.
        if !is_test {
            record_production(cx, item.span.source_callsite(), files);
        }
        return;
    }

    // Hand-written item (`ExpnKind::Root`).
    if let ItemKind::Mod(ident, module) = &item.kind {
        match (cfg_test, is_external_module(cx, item.span, module)) {
            // Inline `#[cfg(test)] mod X { ... }`: one footprint item
            // (its whole block). The body is all test code, so we do not
            // descend into it.
            (true, false) => {
                record_test(cx, item.span, Some(ident.name), files);
            }
            // External `#[cfg(test)] mod X;`: already extracted, neutral
            // for the inline footprint. Its file is a valid extraction
            // target, so we do not descend into it.
            (true, true) => {}
            // Production module: count it and descend into its body
            // (inline children share this file; an external child file
            // gets its own accumulator entry keyed by its own span).
            (false, _) => {
                record_production(cx, item.span, files);
                walk(cx, module, files);
            }
        }
        return;
    }

    // Bare test items (`#[test] fn`, `#[cfg(test)] fn`, any other
    // `#[cfg(test)]` item) contribute to the footprint.
    if is_test {
        record_test(cx, item.span, None, files);
    } else {
        record_production(cx, item.span, files);
    }
}

/// Whether a module's body lives in a different source file than its
/// `mod` declaration — i.e. it is an external `mod X;`, not an inline
/// `mod X { ... }`.
fn is_external_module(cx: &LateContext<'_>, declaration: Span, module: &Mod<'_>) -> bool {
    let source_map = cx.sess().source_map();
    source_map.lookup_source_file(declaration.lo()).start_pos
        != source_map
            .lookup_source_file(module.spans.inner_span.lo())
            .start_pos
}

fn record_production(cx: &LateContext<'_>, span: Span, files: &mut HashMap<BytePos, FileAcc>) {
    acc_for(cx.sess().source_map(), span, files).production_count += 1;
}

fn record_test(
    cx: &LateContext<'_>,
    span: Span,
    module_name: Option<Symbol>,
    files: &mut HashMap<BytePos, FileAcc>,
) {
    let source_map = cx.sess().source_map();
    let lines = line_count(source_map, span);
    let acc = acc_for(source_map, span, files);
    acc.test_items.push(TestItem { span, module_name });
    acc.inline_test_lines += lines;
}

fn acc_for<'a>(
    source_map: &SourceMap,
    span: Span,
    files: &'a mut HashMap<BytePos, FileAcc>,
) -> &'a mut FileAcc {
    let file = source_map.lookup_source_file(span.lo());
    let acc = files.entry(file.start_pos).or_default();
    if acc.file.is_none() {
        acc.file = Some(file);
    }
    acc
}

fn line_count(source_map: &SourceMap, span: Span) -> usize {
    source_map
        .span_to_lines(span)
        .map(|file_lines| file_lines.lines.len())
        .unwrap_or(0)
}

fn emit_inline_style(state: &ExcessiveInlineTests, cx: &LateContext<'_>, acc: &FileAcc) {
    if acc.test_items.is_empty() || acc.production_count == 0 {
        return;
    }
    let Some(file) = &acc.file else {
        return;
    };
    match state.inline_style {
        InlineStyle::ExternalOnly => {
            for item in &acc.test_items {
                span_lint_and_help(
                    cx,
                    EXCESSIVE_INLINE_TESTS,
                    item.span,
                    "inline test code should live in an external module",
                    None,
                    help_extract(file, item.module_name),
                );
            }
        }
        InlineStyle::ExternalWhenLong => {
            let file_lines = file.count_lines();
            let over_absolute = acc.inline_test_lines > state.inline_max_lines;
            let over_fraction = state.inline_max_fraction_of_file.is_some_and(|cap| {
                file_lines > 0 && (acc.inline_test_lines as f32 / file_lines as f32) > cap
            });
            if !over_absolute && !over_fraction {
                return;
            }
            let message = if over_absolute {
                format!(
                    "inline test code spans {} lines, over the limit of {}",
                    acc.inline_test_lines, state.inline_max_lines,
                )
            } else {
                format!(
                    "inline test code is {} of {} lines in this file, over the configured fraction",
                    acc.inline_test_lines, file_lines,
                )
            };
            span_lint_and_help(
                cx,
                EXCESSIVE_INLINE_TESTS,
                union_span(acc.test_items.iter().map(|item| item.span)),
                message,
                None,
                help_extract(file, common_module_name(&acc.test_items)),
            );
        }
    }
}

/// The single module name shared by every recorded test item, if they
/// agree — e.g. a file with one over-budget `#[cfg(test)] mod tests`.
/// `None` when the items disagree or any is a bare item, so the help
/// falls back to the conventional `tests`.
fn common_module_name(items: &[TestItem]) -> Option<Symbol> {
    let mut names = items.iter().map(|item| item.module_name);
    let first = names.next()??;
    names.all(|name| name == Some(first)).then_some(first)
}

/// Help text naming the canonical extraction target. `module_name` is
/// the inline module's own identifier when known; bare test items have
/// none and fall back to the conventional `tests`.
fn help_extract(file: &SourceFile, module_name: Option<Symbol>) -> String {
    let name = module_name.map_or_else(|| "tests".to_owned(), |name| name.to_string());
    match paths::real_path(file).and_then(|path| paths::canonical_target(&path, &name)) {
        Some(target) => format!(
            "move the inline test code into a separate file (e.g. `{}`) and replace it with an \
             external `mod {name};` declaration",
            target.display(),
        ),
        None => format!(
            "move the inline test code into a separate file and replace it with an external \
             `mod {name};` declaration",
        ),
    }
}

fn union_span(spans: impl IntoIterator<Item = Span>) -> Span {
    let mut spans = spans.into_iter();
    let first = spans
        .next()
        .expect("caller guarantees a non-empty iterator");
    spans.fold(first, |union, span| union.to(span))
}
