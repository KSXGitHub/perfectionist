pub fn production(production_param: &str) -> String {
    production_param.to_owned()
}

#[cfg(test)]
mod tests {
    pub fn helper(cfg_test_param: &str) -> String {
        cfg_test_param.to_owned()
    }

    #[test]
    fn nested() {
        fn nested_helper(nested_param: &str) -> String {
            nested_param.to_owned()
        }
        assert_eq!(helper("a"), nested_helper("a"));
    }
}
