//! A module folding two steps into one adapter closure, which
//! `perfectionist::splittable_adapter_closure` splits into an adapter
//! each.

/// A wrapper with two steps worth chaining.
pub struct Wrapper(pub usize);

impl Wrapper {
    /// The wrapped value.
    pub fn value(self) -> usize {
        self.0
    }

    /// The same wrapper with twice the value.
    pub fn doubled(self) -> Wrapper {
        Wrapper(self.0 * 2)
    }
}

/// Each wrapper doubled and then unwrapped, both steps in one closure.
pub fn doubled_values(items: Vec<Wrapper>) -> Vec<usize> {
    items
        .into_iter()
        .map(|wrapper| wrapper.doubled().value())
        .collect()
}

/// What this module contributes to the fixture's output.
pub fn report() -> String {
    let values = doubled_values(vec![Wrapper(1), Wrapper(2)]);
    format!("splittable_adapter_closure: {values:?}\n")
}

#[cfg(test)]
mod tests {
    /// Both steps are applied in the same order to the same items,
    /// which splitting them across two adapters cannot change.
    #[test]
    fn both_steps_still_apply_in_order() {
        let values = super::doubled_values(vec![super::Wrapper(1), super::Wrapper(3)]);
        assert_eq!(values, [2, 6]);
    }
}
