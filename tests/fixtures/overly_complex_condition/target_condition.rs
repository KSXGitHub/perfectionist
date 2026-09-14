// A condition of four operators, one above the default limit. The file
// carries no `#[cfg(test)]` gate and no `#[test]` function: it is test
// code only by virtue of the Cargo target it sits in, which is the half
// of `item_in_test_code` that reads the target rather than an attribute.
pub fn in_target(first: bool, second: bool, third: bool, fourth: bool, fifth: bool) -> bool {
    if first && second && third && fourth && fifth {
        true
    } else {
        false
    }
}

fn main() {}
