fn helper(integration_param: &str) -> String {
    integration_param.to_owned()
}

#[test]
fn works() {
    assert_eq!(helper("a"), "a");
}
