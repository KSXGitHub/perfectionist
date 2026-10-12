use super::AS_PREFIX;
use crate::getter_name_patterns::CONVERSION_PREFIXES;

/// This rule and `cloning_getter` divide the field-copy shape by
/// name. This rule fires on `AS_PREFIX`; `cloning_getter` declines
/// to read any name under `CONVERSION_PREFIXES` as a getter. Were
/// the string to fall out of that list, `cloning_getter` would
/// measure the same methods this rule does.
#[test]
fn the_measured_prefix_is_one_the_getter_rule_refuses() {
    assert!(
        CONVERSION_PREFIXES.contains(&AS_PREFIX),
        "`{AS_PREFIX}` must stay in `CONVERSION_PREFIXES`, or `cloning_getter` would measure \
         the same methods this rule does",
    );
}
