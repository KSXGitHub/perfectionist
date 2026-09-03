fn helper(benchmark_param: &str) -> String {
    benchmark_param.to_owned()
}

#[test]
fn works() {
    assert_eq!(helper("a"), "a");
}
