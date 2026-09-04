//! Sorting one item of a module body into the section it belongs to,
//! and wording the diagnostic when it lands in the wrong one.
//!
//! [`Category`] is the classification; [`Category::rank`] is what the
//! driver compares. Two categories share the last rank, so the ordering
//! the rule enforces has three sections, not four — a private import
//! and a plain item may appear in either order. Keeping them apart as
//! categories is what lets a diagnostic say which of the two blocked
//! the offending item.

use crate::attr_tokens::is_cfg_gated;
use rustc_ast::{Item, ItemKind, VisibilityKind};

/// Where one top-level item of a module body belongs in the sequence
/// the rule enforces.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Category {
    /// A `mod` declaration carrying an explicit visibility.
    PubMod,
    /// A `use` re-export carrying an explicit visibility.
    PubUse,
    /// A `use` import with no explicit visibility.
    PrivateUse,
    /// Every other item: a `fn`, a `struct`, a private `mod`, a
    /// `macro_rules!` definition, an unexpanded macro invocation.
    Other,
}

impl Category {
    /// Which section the category sits in. A smaller rank belongs
    /// further up the module body, and an item whose rank is strictly
    /// below the highest rank already seen is a violation.
    ///
    /// [`Category::PrivateUse`] and [`Category::Other`] share the last
    /// rank: the enforced order puts private imports and other items in
    /// one trailing section without ordering them against each other.
    pub(super) fn rank(self) -> u8 {
        match self {
            Category::PubMod => 0,
            Category::PubUse => 1,
            Category::PrivateUse | Category::Other => 2,
        }
    }

    /// How a diagnostic names an item of this category. Worded to read
    /// after an indefinite article, so both the warning header and the
    /// note below it can splice it in.
    pub(super) fn subject(self) -> &'static str {
        match self {
            Category::PubMod => "`pub mod` declaration",
            Category::PubUse => "`pub use` re-export",
            Category::PrivateUse => "private `use` import",
            Category::Other => "non-import item",
        }
    }
}

/// Which section `item` belongs to, or `None` when it is transparent to
/// the ordering — neither flagged nor closing the section it sits in.
///
/// Three shapes are transparent:
///
/// - **`extern crate`.** `#[macro_use] extern crate foo;` has to sit at
///   the top of a crate root, above the `pub mod` declarations, and
///   predates the imports the rule orders.
/// - **A `#[cfg(...)]`-gated `use`.** A conditional import block is
///   conventionally parked below the unconditional ones, so gating it
///   excuses it from the ordering rather than pinning what follows.
/// - **A macro-expanded item.** The rule reads the layout the author
///   wrote; an item an expansion produced is not one of those.
pub(super) fn classify(item: &Item) -> Option<Category> {
    if item.span.from_expansion() {
        return None;
    }
    match &item.kind {
        ItemKind::ExternCrate(..) => None,
        ItemKind::Use(_) if is_cfg_gated(&item.attrs) => None,
        ItemKind::Use(_) if has_explicit_visibility(item) => Some(Category::PubUse),
        ItemKind::Use(_) => Some(Category::PrivateUse),
        ItemKind::Mod(..) if has_explicit_visibility(item) => Some(Category::PubMod),
        _ => Some(Category::Other),
    }
}

/// Whether the item is written with a visibility of its own. Any
/// explicit visibility counts, so `pub(crate) mod` and `pub(super) use`
/// are ordered as a `pub mod` declaration and a `pub use` re-export —
/// the same reading `perfectionist::import_grouping_mismatch` gives a
/// re-export. Only the inherited (unwritten) visibility is private.
fn has_explicit_visibility(item: &Item) -> bool {
    !matches!(item.vis.kind, VisibilityKind::Inherited)
}
