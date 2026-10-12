use super::claim;
use std::sync::Mutex;

#[test]
fn a_name_is_claimed_once() {
    claim(&Mutex::default(), "claimed-once");
}

#[test]
#[should_panic(expected = "already taken in this test binary")]
fn claiming_a_name_twice_panics() {
    let taken = Mutex::default();
    claim(&taken, "claimed-twice");
    claim(&taken, "claimed-twice");
}

#[test]
fn two_names_do_not_collide() {
    let taken = Mutex::default();
    claim(&taken, "distinct-left");
    claim(&taken, "distinct-right");
}

#[test]
fn one_name_in_two_registries_does_not_collide() {
    claim(&Mutex::default(), "shared-name");
    claim(&Mutex::default(), "shared-name");
}
