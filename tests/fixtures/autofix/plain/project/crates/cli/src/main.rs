//! Print what every module of this fixture contributes, so a rewrite
//! that changes what the project does shows up as changed output.
//!
//! Each module is named by its full path rather than through a `use`,
//! so the import rules have nothing to rewrite here and this file
//! reads the same on every run. The modules are printed in
//! the order the rules they stand for are named, which is the order
//! `src/rule_index.rs` lists them in.

fn main() {
    print!("{}", shapes::import_granularity_mismatch::report());
    print!("{}", shapes::macro_trailing_comma::report());
    print!("{}", named_prelude_imports::report());
}
