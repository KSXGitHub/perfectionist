use super::claim;

#[test]
fn a_name_is_claimed_once() {
    claim("claimed-once");
}

#[test]
#[should_panic(expected = "already taken in this test binary")]
fn claiming_a_name_twice_panics() {
    claim("claimed-twice");
    claim("claimed-twice");
}

#[test]
fn two_names_do_not_collide() {
    claim("distinct-left");
    claim("distinct-right");
}
