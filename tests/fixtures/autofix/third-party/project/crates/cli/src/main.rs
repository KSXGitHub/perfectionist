//! Print what every module of this fixture contributes, so a rewrite
//! that changes what the project does shows up as changed output.
//!
//! Each module is named by its full path rather than through a `use`,
//! so the import rules have nothing to rewrite here.

fn main() {
    print!("{}", shapes::folded_command_setter::report());
    print!("{}", shapes::mutating_command_builder::report());
    print!("{}", redundant_derive_more_forward_template::report());
}
