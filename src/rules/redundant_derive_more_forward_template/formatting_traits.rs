//! The table tying each `derive_more` formatting trait to the three
//! spellings the rule has to line up: the derive that implements it, the
//! helper attribute that configures it, and the format-spec type that
//! selects it in a placeholder.

use rustc_span::Symbol;

/// One `derive_more` formatting trait, in the three spellings the rule
/// has to line up: the derive that implements it, the helper attribute
/// that configures it, and the format-spec type that selects it in a
/// placeholder.
pub(super) struct FormattingTrait {
    /// Final path segment of the derive, as written in `#[derive(...)]`.
    pub(super) derive: &'static str,
    /// The derive's helper attribute, always written unqualified.
    pub(super) attribute: &'static str,
    /// The placeholder type that selects this trait — `""` for
    /// `Display`, whose placeholder carries no type at all.
    pub(super) spec_type: &'static str,
}

/// Every `derive_more` derive whose no-attribute default is a forward
/// to the container's single field.
///
/// `Debug` is deliberately absent: its default is the struct-shaped
/// `Wrapper("inner")` builder output rather than a forward, so a
/// `#[debug("{_0:?}")]` genuinely changes the rendering.
const FORMATTING_TRAITS: &[FormattingTrait] = &[
    FormattingTrait {
        derive: "Binary",
        attribute: "binary",
        spec_type: "b",
    },
    FormattingTrait {
        derive: "Display",
        attribute: "display",
        spec_type: "",
    },
    FormattingTrait {
        derive: "LowerExp",
        attribute: "lower_exp",
        spec_type: "e",
    },
    FormattingTrait {
        derive: "LowerHex",
        attribute: "lower_hex",
        spec_type: "x",
    },
    FormattingTrait {
        derive: "Pointer",
        attribute: "pointer",
        spec_type: "p",
    },
    FormattingTrait {
        derive: "Octal",
        attribute: "octal",
        spec_type: "o",
    },
    FormattingTrait {
        derive: "UpperExp",
        attribute: "upper_exp",
        spec_type: "E",
    },
    FormattingTrait {
        derive: "UpperHex",
        attribute: "upper_hex",
        spec_type: "X",
    },
];

/// The formatting trait whose helper attribute is named `name`.
pub(super) fn formatting_trait(name: Symbol) -> Option<&'static FormattingTrait> {
    FORMATTING_TRAITS
        .iter()
        .find(|entry| name == Symbol::intern(entry.attribute))
}
