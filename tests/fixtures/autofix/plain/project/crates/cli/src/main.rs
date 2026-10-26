//! Print what every crate in this fixture contributes, so a rewrite
//! that changes what the project does shows up as changed output.
//!
//! Each crate is named by its full path rather than through a `use`,
//! so the import rules have nothing to rewrite here and this file
//! reads the same on every run. The crates are printed in
//! the order the rules they stand for are named, which is the order
//! `src/rule_index.rs` lists them in.

fn main() {
    print!("{}", import_granularity_mismatch::report());
    print!("{}", macro_trailing_comma::report());
    print!("{}", named_prelude_imports::report());
}
