use std::collections::HashSet;

pub fn production(names: Vec<String>) -> Vec<String> {
    names.into_iter().collect::<HashSet<_>>().into_iter().collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    pub fn cfg_test_helper(names: Vec<String>) -> Vec<String> {
        names.into_iter().collect::<HashSet<_>>().into_iter().collect()
    }

    #[test]
    fn test_function() {
        let names = vec!["one".to_string(), "one".to_string()];
        let unique: Vec<String> =
            names.clone().into_iter().collect::<HashSet<_>>().into_iter().collect();
        assert_eq!(unique.len(), cfg_test_helper(names).len());
    }
}
