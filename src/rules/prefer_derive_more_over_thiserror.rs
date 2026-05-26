//! `perfectionist::prefer_derive_more_over_thiserror` — flag every
//! `thiserror` import, derive, or attribute and steer toward
//! `derive_more::{Display, Error}`.
//!
//! This file owns the lint declaration, the registration functions,
//! and the pre-expansion `EarlyLintPass` driver. The work is split
//! across submodules:
//! - [`config`] — `Config`, the `PreferDeriveMoreOverThiserror` pass
//!   state, and the configured-path matcher.
//! - [`scan`] — crate-wide `use` / `extern crate` alias collection.
//! - [`detect`] — per-item derive / attribute / import matching and
//!   diagnostic dispatch.
//! - [`emit`] — the diagnostic-emission helpers.

use rustc_ast::{Crate, Item, ItemKind};
use rustc_lint::{EarlyContext, EarlyLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

mod config;
mod detect;
mod emit;
mod scan;

use config::PreferDeriveMoreOverThiserror;

use crate::common::{DefaultState, resolved_state};

declare_tool_lint! {
    /// ### What it does
    /// Flags every use of [`thiserror`](https://docs.rs/thiserror) in
    /// the consumer crate. Three syntactic shapes trigger the lint:
    ///
    /// 1. **Derives.** `#[derive(thiserror::Error)]` directly, or
    ///    `#[derive(Error)]` / `#[derive(te::Error)]` when a sibling
    ///    `use thiserror::Error;` / `use thiserror as te;` brings the
    ///    derive macro into scope under any local name, anywhere in
    ///    the crate. `#[cfg_attr(_, derive(thiserror::Error))]` is
    ///    unwrapped (including nested `cfg_attr`).
    /// 2. **Attributes.** `#[error(...)]` attributes attached to an
    ///    item the rule has already classified as thiserror-derived,
    ///    on the item, its enum variants, or its fields.
    ///    `#[cfg_attr(_, error(...))]` is unwrapped symmetrically
    ///    with the derive side.
    /// 3. **Imports.** Every `use` or `extern crate` statement that
    ///    brings a `thiserror` path into scope:
    ///    `use thiserror::*`, `use thiserror::Error`,
    ///    `use thiserror::Error as MyError;`,
    ///    `use thiserror::{self as te};`, `use thiserror as te;`,
    ///    `extern crate thiserror;`, `extern crate thiserror as te;`,
    ///    the braced top-level form `use {thiserror::Error, ...};`,
    ///    and `pub use` re-exports.
    ///
    /// The lint is detection-only: it emits a help-style diagnostic
    /// pointing at the offending site and suggests migrating to
    /// `#[derive(derive_more::Display, derive_more::Error)]`. There is
    /// no autofix — the migration involves a mix of derive-list edits,
    /// format-string positional translation (`thiserror`'s `{0}` ↔
    /// `derive_more`'s `{_0}`), attribute renames (`#[error(...)]` ↔
    /// `#[display(...)]`), and edge cases (`#[error(transparent)]`,
    /// `#[backtrace]`) whose mechanical rewrite is too risky to apply
    /// without review.
    ///
    /// Because the pass runs pre-expansion and does not consult
    /// name resolution, alias collection is crate-wide rather than
    /// per-module: a `use thiserror::Error;` anywhere in the crate
    /// makes the bare `#[derive(Error)]` short-hand resolve as
    /// thiserror everywhere. In practice that overlap is rare and
    /// the rule treats it as acceptable false-positive surface; a
    /// project that hits it can suppress individual sites with
    /// `#[allow(perfectionist::prefer_derive_more_over_thiserror)]`.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. The
    /// catalogue picks `derive_more` for error formatting and source
    /// chaining. Mixing in `thiserror` fragments the attribute
    /// vocabulary across the codebase and adds a second derive crate
    /// that has no functional capability `derive_more` lacks. A
    /// project that wants the choice the other way around can disable
    /// this rule.
    ///
    /// ### Example
    /// ```rust,ignore
    /// use thiserror::Error;
    ///
    /// #[derive(Debug, Error)]
    /// pub enum MyError {
    ///     #[error("missing field {0}")]
    ///     MissingField(String),
    /// }
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// use derive_more::{Display, Error};
    ///
    /// #[derive(Debug, Display, Error)]
    /// pub enum MyError {
    ///     #[display("missing field {_0}")]
    ///     MissingField(String),
    /// }
    /// ```
    pub perfectionist::PREFER_DERIVE_MORE_OVER_THISERROR,
    Warn,
    "`thiserror` import, derive, or attribute; this catalogue prefers `derive_more::{Display, Error}`",
    report_in_external_macro: false
}

/// Active by default. Read by `register_pass` below; gen-docs picks
/// the constant up via syn to render the rule's default state.
pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Active;

impl_lint_pass!(PreferDeriveMoreOverThiserror => [PREFER_DERIVE_MORE_OVER_THISERROR]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[PREFER_DERIVE_MORE_OVER_THISERROR]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive =
        resolved_state("prefer_derive_more_over_thiserror", DEFAULT_STATE)
    {
        return;
    }
    // Pre-expansion: derives are consumed during macro expansion, so a
    // regular (post-expansion) pass no longer sees the
    // `#[derive(...)]` attribute by the time the rule looks for it.
    // The sibling `perfectionist::derive_ordering` rule uses the same
    // hook for the same reason.
    lint_store.register_pre_expansion_pass(|| Box::new(PreferDeriveMoreOverThiserror::new()));
}

impl EarlyLintPass for PreferDeriveMoreOverThiserror {
    fn check_crate(&mut self, _cx: &EarlyContext<'_>, krate: &Crate) {
        scan::collect_aliases(self, krate);
    }

    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        match &item.kind {
            ItemKind::Use(use_tree) => self.check_use(cx, item, use_tree),
            ItemKind::ExternCrate(orig, ident) => {
                // `extern crate thiserror;` — `orig` is `None`,
                // `ident.name` is the crate name. `extern crate
                // thiserror as te;` — `orig` is `Some(thiserror)`,
                // `ident.name` is `te`. The alias side is recorded by
                // the scan; emit the import diagnostic on the item
                // span here.
                let original = orig.unwrap_or(ident.name);
                if self.thiserror_crates.contains(&original) {
                    emit::emit_use(cx, item.span);
                }
            }
            ItemKind::Struct(_, _, data) => self.check_struct(cx, &item.attrs, data),
            ItemKind::Enum(_, _, def) => self.check_enum(cx, &item.attrs, def),
            _ => {}
        }
    }
}
