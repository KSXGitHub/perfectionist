use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::emit_at_enclosing_hir;
use crate::module_reparse::parse_crate_module_files;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::source_map::SourceMap;
use rustc_span::{BytePos, Span};

mod attrs;
mod collect;

use collect::{ForwardKind, Violation, collect_violations};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a `derive_more` formatting attribute whose template is
    /// nothing but the forward the derive already performs, and
    /// suggests deleting it.
    ///
    /// A formatting derive on a container with exactly one field
    /// forwards to that field, so a template that does nothing but
    /// name that field says nothing the derive does not — written
    /// inline (`#[display("{_0}")]`,
    /// `#[display("{the_only_field}")]`) or as an argument
    /// (`#[display("{}", _0)]`, `#[display("{}", self.0)]`), on a
    /// struct or on a single-field enum variant. An enum-level
    /// `#[display("{_variant}")]` is the container-level counterpart:
    /// it names exactly what each variant would be formatted with
    /// anyway.
    ///
    /// Each of `derive_more`'s formatting derives is read through its
    /// own helper attribute — `Display` through `#[display(...)]`,
    /// `LowerHex` through `#[lower_hex(...)]`, and so on — but the
    /// trait a template forwards to comes from the *placeholder*, not
    /// from the attribute's name. So `#[lower_hex("{_0}")]` stays
    /// unflagged: a bare `{}` forwards to `Display`, which is not
    /// what the `LowerHex` derive would have done.
    ///
    /// `#[debug(...)]` is never flagged. `derive_more`'s `Debug`
    /// derive defaults to the struct-shaped `Wrapper("inner")` output
    /// rather than to a forward, so its template always changes the
    /// rendering.
    ///
    /// Beyond that the rule is silent wherever deleting the attribute
    /// would change the output — among them:
    ///
    /// - An adorned placeholder: `#[display("{_0:>8}")]` applies its
    ///   own width instead of passing the caller's format spec
    ///   through.
    /// - A variant under an enum-level template that does not mention
    ///   `{_variant}`, which is what the variant would fall back to.
    /// - A `cfg`-gated field, or a template inside a
    ///   `#[cfg_attr(...)]`: the field count may differ between
    ///   configurations.
    ///
    /// A derive renamed on import (`use derive_more::Display as D;`)
    /// is not recognised; a re-export under the same name is.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue: the
    /// attribute compiles to exactly the code the derive emits without
    /// it. It is the same shape of dead weight as an unused import or
    /// a needless borrow — it restates what the compiler already does,
    /// and a reader has to read the whole template before concluding
    /// it changes nothing.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// #[derive(derive_more::Display)]
    /// #[display("{_0}")]
    /// struct SanitizedHtml(String);
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// #[derive(derive_more::Display)]
    /// struct SanitizedHtml(String);
    /// ```
    pub perfectionist::REDUNDANT_DERIVE_MORE_FORWARD_TEMPLATE,
    Warn,
    "`derive_more` formatting template only restates the forward the derive already performs",
    report_in_external_macro: false
}

/// Active by default. Read by [`register_pass`] below; gen-docs picks
/// the constant up via syn to render the rule's default state.
pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Active;

const CONFIG_KEY: &str = "perfectionist::redundant_derive_more_forward_template";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct RedundantDeriveMoreForwardTemplate;

impl RedundantDeriveMoreForwardTemplate {
    fn new() -> Self {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self
    }
}

impl_lint_pass!(RedundantDeriveMoreForwardTemplate => [REDUNDANT_DERIVE_MORE_FORWARD_TEMPLATE]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[REDUNDANT_DERIVE_MORE_FORWARD_TEMPLATE]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive =
        resolved_state("redundant_derive_more_forward_template", DEFAULT_STATE)
    {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(RedundantDeriveMoreForwardTemplate::new()));
}

impl<'tcx> LateLintPass<'tcx> for RedundantDeriveMoreForwardTemplate {
    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        // Re-parse the crate's module files so the `#[derive(...)]`
        // list and the formatting attribute survive (macro expansion
        // has consumed both by the late pass) and every separate-file
        // submodule is reached.
        let (crates, live_module_spans) = parse_crate_module_files(cx);
        let violations = collect_violations(&crates, &live_module_spans);
        // No `hir_in_external_macro` guard: the diagnostic span is the
        // whole attribute rather than a bare identifier inside it, so
        // the built-in `report_in_external_macro: false` filter already
        // covers this rule per the "vulnerable exactly when" test in
        // `planned-rules/IMPLEMENTATION_CONVENTIONS.md`. The omission is
        // deliberate.
        // Anchored through `emit_at_enclosing_hir` rather than the
        // plain `find_enclosing_hir_ids`: only the former registers enum
        // variants as anchors, and a variant is where a finding on a
        // variant's attribute has to land for a per-variant `#[allow]`
        // to silence just that variant.
        let anchored = violations
            .into_iter()
            .map(|violation| (violation.anchor, violation))
            .collect();
        emit_at_enclosing_hir(cx.tcx, anchored, |hir_id, _anchor, violation| {
            emit(cx, hir_id, &violation);
        });
    }
}

fn emit(cx: &LateContext<'_>, hir_id: hir::HirId, violation: &Violation) {
    let Violation {
        attribute,
        kind,
        attribute_name,
        derive_name,
        ..
    } = violation;
    let message = match kind {
        ForwardKind::SingleField => format!(
            "`#[{attribute_name}(...)]` only forwards to the single field, \
             which the `{derive_name}` derive already does",
        ),
        ForwardKind::Variant => format!(
            "`#[{attribute_name}(\"{{_variant}}\")]` only restates how the \
             `{derive_name}` derive already formats every variant",
        ),
    };
    span_lint_hir_and_then(
        cx,
        REDUNDANT_DERIVE_MORE_FORWARD_TEMPLATE,
        hir_id,
        *attribute,
        message,
        |diagnostic| {
            diagnostic.span_suggestion(
                deletion_span(cx.sess().source_map(), *attribute),
                "remove the attribute",
                String::new(),
                Applicability::MachineApplicable,
            );
        },
    );
}

/// The span to delete for an attribute that reaches the fix. An
/// attribute alone on its line takes the whole line with it, newline
/// included, so no blank line is left behind; one sharing its line with
/// the item it annotates takes only itself and the spaces after it.
fn deletion_span(source_map: &SourceMap, attribute: Span) -> Span {
    let Ok(lines) = source_map.span_to_lines(attribute) else {
        return attribute;
    };
    let (Some(first), Some(last)) = (lines.lines.first(), lines.lines.last()) else {
        return attribute;
    };
    let file = &lines.file;
    let (Some(first_text), Some(last_text)) = (
        file.get_line(first.line_index),
        file.get_line(last.line_index),
    ) else {
        return attribute;
    };
    let indent = first_text.chars().take(first.start_col.0);
    let after: String = last_text.chars().skip(last.end_col.0).collect();
    if indent.chain(after.chars()).all(char::is_whitespace) {
        // `line_bounds` ends past the line terminator, so the whole
        // line goes with the attribute and no blank line is left.
        let start = file.line_bounds(first.line_index).start;
        let end = file.line_bounds(last.line_index).end;
        return attribute.with_lo(start).with_hi(end);
    }
    // The attribute shares its line with the item it annotates, so take
    // only the spaces separating the two.
    let separator = after
        .bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count();
    attribute.with_hi(attribute.hi() + BytePos(separator as u32))
}
