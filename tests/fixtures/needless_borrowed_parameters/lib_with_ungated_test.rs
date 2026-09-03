// No `#[cfg(test)]` anywhere: rustc drops a `#[test]` function whole
// in a non-test build, so this helper exists only under `cfg(test)`
// and only `is_in_test_function` can recognise it as test code.
#[test]
fn ungated() {
    fn helper(ungated_test_param: &str) -> String {
        ungated_test_param.to_owned()
    }
    assert_eq!(helper("a"), "a");
}
