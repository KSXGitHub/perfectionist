//! Print what every crate in this fixture contributes, so a rewrite
//! that changes what the codebase does shows up as changed output.
//!
//! Each crate is named by its full path rather than through a `use`,
//! so the import rules have nothing to rewrite here.

fn main() {
    print!("{}", folded_command_setter::report());
    print!("{}", mutating_command_builder::report());
    print!("{}", redundant_derive_more_forward_template::report());
}
