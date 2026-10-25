//! Every shape whose rule reads one item, one module each.
//!
//! They share a crate because every linter and every fixer pays a
//! fixed cost per crate. What earns a crate of its own is a rule that
//! reads the project's layout, a dependency another module must not
//! see, or a proc macro.

pub mod avoidable_string_escapes;
pub mod bare_email;
pub mod bare_url;
pub mod clap_help_markdown;
pub mod core_instead_of_std;
pub mod import_granularity_mismatch;
pub mod macro_trailing_comma;
pub mod map_or;
pub mod numeric_escape_for_visible_character;
pub mod overly_long_print_macro;
pub mod some_bool_comparison;
pub mod splittable_adapter_closure;
pub mod unordered_derives;
