//! Every shape whose rule reads one item, one module each.
//!
//! They share a crate because every linter and every fixer pays a
//! fixed cost per crate. What earns a crate of its own is a rule that
//! reads the project's layout, a dependency another module must not
//! see, or a proc macro.

pub mod import_granularity_mismatch;
pub mod macro_trailing_comma;
