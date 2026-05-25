//! The per-crate HIR walk. Classifies every item as production or
//! test, accumulates each source file's inline-test footprint, drives
//! the external-module layout check, and emits the inline-style
//! diagnostics once per file.

use std::collections::HashMap;
use std::sync::Arc;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::{is_cfg_test, is_test_function};
use rustc_hir::{Item, ItemKind, Mod};
use rustc_lint::{LateContext, LintContext};
use rustc_span::source_map::SourceMap;
use rustc_span::{BytePos, SourceFile, Span};

use super::UNIT_TEST_FILE_LAYOUT;
use super::config::{InlineStyle, UnitTestFileLayout};
use super::layout;

/// Per-source-file tally built up during the walk.
#[derive(Default)]
struct FileAcc {
    /// Top-level production items seen in this file. A file with zero
    /// of these is entirely test code and exempt from the inline-style
    /// check — it is itself a valid extraction target.
    production_count: usize,
    /// Spans of the inline test items contributing to the footprint.
    test_item_spans: Vec<Span>,
    /// Sum of the line spans of every item in `test_item_spans`.
    inline_test_lines: usize,
    /// The file these tallies belong to, kept for line counting and its
    /// path. `None` only before the first item is recorded.
    file: Option<Arc<SourceFile>>,
}

pub(super) fn run(state: &UnitTestFileLayout, cx: &LateContext<'_>) {
    let mut files: HashMap<BytePos, FileAcc> = HashMap::new();
    walk(state, cx, cx.tcx.hir_root_module(), &mut files);
    for acc in files.values() {
        emit_inline_style(state, cx, acc);
    }
}

fn walk(
    state: &UnitTestFileLayout,
    cx: &LateContext<'_>,
    module: &Mod<'_>,
    files: &mut HashMap<BytePos, FileAcc>,
) {
    for item_id in module.item_ids {
        classify(state, cx, cx.tcx.hir_item(*item_id), files);
    }
}

fn classify(
    state: &UnitTestFileLayout,
    cx: &LateContext<'_>,
    item: &Item<'_>,
    files: &mut HashMap<BytePos, FileAcc>,
) {
    let cfg_test = is_cfg_test(cx.tcx, item.hir_id());
    if let ItemKind::Mod(ident, module) = &item.kind {
        match (cfg_test, is_external_module(cx, item.span, module)) {
            // Inline `#[cfg(test)] mod X { ... }`: one footprint item
            // (its whole block), counted when its name is in scope. The
            // body is all test code, so we do not descend into it.
            (true, false) => {
                if state.module_name_in_scope(ident.name) {
                    record_test(cx, item.span, files);
                }
            }
            // External `#[cfg(test)] mod X;`: layout-checked, neutral
            // for the inline footprint. Its file is a valid extraction
            // target, so we do not descend into it.
            (true, true) => {
                layout::check_external_mod(state, cx, item, ident.name, module);
            }
            // Production module: count it and descend into its body
            // (inline children share this file; an external child file
            // gets its own accumulator entry keyed by its own span).
            (false, _) => {
                record_production(cx, item.span, files);
                walk(state, cx, module, files);
            }
        }
        return;
    }

    // Bare test items (`#[test] fn`, `#[cfg(test)] fn`, any other
    // `#[cfg(test)]` item) contribute to the footprint only when no
    // `test_module_names` filter narrows it to named modules.
    let is_test_item = cfg_test
        || (matches!(item.kind, ItemKind::Fn { .. })
            && is_test_function(cx.tcx, item.owner_id.def_id));
    if is_test_item {
        if state.test_module_names.is_empty() {
            record_test(cx, item.span, files);
        }
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

fn record_test(cx: &LateContext<'_>, span: Span, files: &mut HashMap<BytePos, FileAcc>) {
    let source_map = cx.sess().source_map();
    let lines = line_count(source_map, span);
    let acc = acc_for(source_map, span, files);
    acc.test_item_spans.push(span);
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

fn emit_inline_style(state: &UnitTestFileLayout, cx: &LateContext<'_>, acc: &FileAcc) {
    if acc.test_item_spans.is_empty() || acc.production_count == 0 {
        return;
    }
    let Some(file) = &acc.file else {
        return;
    };
    match state.inline_style {
        InlineStyle::ExternalOnly => {
            let help = help_extract(state, file);
            for &span in &acc.test_item_spans {
                span_lint_and_help(
                    cx,
                    UNIT_TEST_FILE_LAYOUT,
                    span,
                    "inline test code should live in an external module",
                    None,
                    help.clone(),
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
                UNIT_TEST_FILE_LAYOUT,
                union_span(&acc.test_item_spans),
                message,
                None,
                help_extract(state, file),
            );
        }
    }
}

fn help_extract(state: &UnitTestFileLayout, file: &SourceFile) -> String {
    match layout::real_path(file)
        .and_then(|path| layout::canonical_target(&path, "tests", state.external_layout))
    {
        Some(target) => format!(
            "move the inline test code into `{}` and declare `mod tests;` in its place",
            target.display(),
        ),
        None => {
            "move the inline test code into an external module declared as `mod tests;`".to_owned()
        }
    }
}

fn union_span(spans: &[Span]) -> Span {
    let mut iter = spans.iter().copied();
    let first = iter.next().expect("caller guarantees a non-empty slice");
    iter.fold(first, |union, span| union.to(span))
}
