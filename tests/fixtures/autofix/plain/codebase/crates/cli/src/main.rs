//! Print what every crate in this fixture contributes, so a rewrite
//! that changes what the codebase does shows up as changed output.
//!
//! Each crate is named by its full path rather than through a `use`,
//! so the import rules have nothing to rewrite here and this file
//! reads the same on every settling path. The crates are printed in
//! the order the rules they stand for are named, which is the order
//! `src/rule_index.rs` lists them in.

fn main() {
    print!("{}", avoidable_string_escapes::report());
    print!("{}", bare_email::report());
    print!("{}", bare_url::report());
    print!("{}", clap_help_markdown::report());
    print!("{}", core_instead_of_std::report());
    print!("{}", import_granularity_mismatch::report());
    print!("{}", import_grouping_mismatch::report());
    print!("{}", macro_trailing_comma::report());
    print!("{}", map_or::report());
    print!("{}", named_prelude_imports::report());
    print!("{}", numeric_escape_for_visible_character::report());
    print!("{}", overly_long_print_macro::report());
    print!("{}", some_bool_comparison::report());
    print!("{}", splittable_adapter_closure::report());
    print!("{}", uncombined_self_import::report());
    print!("{}", unordered_derives::report());
}
