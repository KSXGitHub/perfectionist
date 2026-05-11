//! Render the per-rule Configuration block: the `<details>` panel
//! listing each TOML key, plus the per-type sub-blocks describing
//! the shape of any project-local enums or structs the fields reach.

use maud::{Markup, PreEscaped, html};

use crate::model::{ConfigDoc, TypeDoc, TypeKind};
use crate::render::markdown::markdown_to_html;

/// Render the per-rule configuration block. Two shapes:
///
/// 1. When the rule has at least one configurable field, wrap the
///    block in a `<details>` element, collapsed by default. The
///    rule's description above is what most catalogue readers came
///    for; configuration is reference material that can stay
///    hidden until clicked. Modern browsers auto-expand a closed
///    `<details>` when find-in-page hits something inside it, so
///    search still works.
/// 2. When the rule has no fields (an empty `Config` struct,
///    e.g. `flat_module_pattern`), render a single inline line
///    "Configuration: none." instead. The fact that the rule is
///    not configurable is itself information; omitting the section
///    entirely would leave readers wondering whether they missed a
///    knob. Any struct-level doc comment is *not* surfaced in this
///    case — an empty config has no user-facing schema to
///    annotate, so the comment is internal implementation prose
///    that belongs in the source, not on the catalogue page.
pub(crate) fn config_section(config: &ConfigDoc) -> Markup {
    if config.fields.is_empty() {
        return html! {
            p.config-none {
                strong { "Configuration:" } " none."
            }
        };
    }
    html! {
        details.config-details {
            summary.config-summary { "Configuration" }
            p {
                "Configure via " code { "dylint.toml" } " under "
                code { "[\"" (config.key) "\"]" } "."
            }
            dl.config {
                @for field in &config.fields {
                    dt {
                        code.config-key { (field.name) }
                        " : "
                        code.config-type { (field.type_label) }
                        " "
                        span.badge.badge-optional { "optional" }
                    }
                    dd {
                        @if field.doc_markdown.is_empty() {
                            p { em { "Undocumented." } }
                        } @else {
                            (PreEscaped(markdown_to_html(&field.doc_markdown)))
                        }
                    }
                }
            }
            @if !config.custom_types.is_empty() {
                h4 { "Types" }
                @for ty in &config.custom_types {
                    (custom_type_block(ty))
                }
            }
        }
    }
}

/// Render one custom type referenced by a `Config` field. Enum
/// variants are listed with the string a TOML author would write,
/// since that's the user-facing value; the Rust identifier is shown
/// in parentheses only when it differs.
fn custom_type_block(ty: &TypeDoc) -> Markup {
    let kind_label = match ty.kind {
        TypeKind::Enum { .. } => "enum",
        TypeKind::Struct { .. } => "struct",
    };
    html! {
        div.custom-type {
            p {
                code.custom-type-name { (ty.name) }
                " "
                span.custom-type-kind { (kind_label) }
            }
            @if !ty.doc_markdown.is_empty() {
                (PreEscaped(markdown_to_html(&ty.doc_markdown)))
            }
            @match &ty.kind {
                TypeKind::Enum { variants } => {
                    @if !variants.is_empty() {
                        dl.config {
                            @for variant in variants {
                                dt {
                                    code.config-key { "\"" (variant.serialized) "\"" }
                                    @if variant.rust_name != variant.serialized {
                                        " "
                                        span.config-default {
                                            "(Rust: " code { (variant.rust_name) } ")"
                                        }
                                    }
                                }
                                dd {
                                    @if variant.doc_markdown.is_empty() {
                                        p { em { "Undocumented." } }
                                    } @else {
                                        (PreEscaped(markdown_to_html(&variant.doc_markdown)))
                                    }
                                }
                            }
                        }
                    }
                }
                TypeKind::Struct { fields } => {
                    @if !fields.is_empty() {
                        dl.config {
                            @for field in fields {
                                dt {
                                    code.config-key { (field.name) }
                                    " : "
                                    code.config-type { (field.type_label) }
                                }
                                dd {
                                    @if field.doc_markdown.is_empty() {
                                        p { em { "Undocumented." } }
                                    } @else {
                                        (PreEscaped(markdown_to_html(&field.doc_markdown)))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
