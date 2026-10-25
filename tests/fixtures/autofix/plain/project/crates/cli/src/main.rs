//! Print what every module of this fixture contributes, so a rewrite
//! that changes what the project does shows up as changed output.
//!
//! Each module is named by its full path rather than through a `use`,
//! so the import rules have nothing to rewrite here and this file
//! reads the same on every run. The modules are printed in
//! the order the rules they stand for are named, which is the order
//! `src/rule_index.rs` lists them in.

fn main() {
    print!("{}", shapes::avoidable_string_escapes::report());
    print!("{}", shapes::bare_email::report());
    print!("{}", shapes::bare_url::report());
    print!("{}", shapes::clap_help_markdown::report());
    print!("{}", shapes::core_instead_of_std::report());
    print!("{}", shapes::import_granularity_mismatch::report());
    print!("{}", import_grouping_mismatch::report());
    print!("{}", shapes::macro_trailing_comma::report());
    print!("{}", shapes::map_or::report());
    print!("{}", named_prelude_imports::report());
    print!("{}", shapes::numeric_escape_for_visible_character::report());
    print!("{}", shapes::overly_long_print_macro::report());
    print!("{}", shapes::some_bool_comparison::report());
    print!("{}", shapes::splittable_adapter_closure::report());
    print!("{}", uncombined_self_import::report());
    print!("{}", shapes::unordered_derives::report());
}
