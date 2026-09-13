// A struct of eleven fields, one above the default limit. The file
// carries no `#[cfg(test)]` gate and no `#[test]` function: it is test
// code only by virtue of the Cargo target it sits in, which is the half
// of `item_in_test_code` that reads the target rather than an attribute.
pub struct InTarget {
    field_1: u32,
    field_2: u32,
    field_3: u32,
    field_4: u32,
    field_5: u32,
    field_6: u32,
    field_7: u32,
    field_8: u32,
    field_9: u32,
    field_10: u32,
    field_11: u32,
}

fn main() {}
