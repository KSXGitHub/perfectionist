//! A crate importing a module and an item of it in two statements,
//! which `perfectionist::uncombined_self_import` folds into one.
//! The rule ships inactive, so only a run that enables it rewrites
//! this.
//!
//! The module is imported as `{self}` rather than as the bare `use
//! module;`, since folding the bare form changes which namespaces
//! the name is bound in and the rule hands that over at
//! `MaybeIncorrect`.

pub mod defs {
    pub mod inner {
        /// An item the importing scope names through its module.
        pub struct Token(pub u8);

        /// An item the importing scope names directly.
        pub struct Label(pub u8);
    }
}

mod user {
    use crate::defs::inner::{self};
    use crate::defs::inner::Label;

    /// Both items, reached each the way its own `use` statement
    /// imported it.
    pub fn describe() -> String {
        format!("{} {}", inner::Token(1).0, Label(2).0)
    }
}

/// What this crate contributes to the fixture's output.
pub fn report() -> String {
    format!("uncombined_self_import: {}\n", user::describe())
}

#[cfg(test)]
mod tests {
    /// Both items are still reached, which folding the two
    /// statements into one cannot change.
    #[test]
    fn both_items_are_still_reached() {
        assert_eq!(super::report(), "uncombined_self_import: 1 2\n");
    }
}
