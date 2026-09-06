pub fn production(items: &[u32]) -> u32 {
    items
        .iter()
        .copied()
        .map(|item| item + 1)
        .filter(|item| *item > 1)
        .rev()
        .sum::<u32>()
}

#[cfg(test)]
mod tests {
    pub fn cfg_test_helper(items: &[u32]) -> u32 {
        items
            .iter()
            .copied()
            .map(|item| item + 1)
            .filter(|item| *item > 1)
            .rev()
            .sum::<u32>()
    }

    #[test]
    fn test_function() {
        let items = [1, 2];
        let total = items
            .iter()
            .copied()
            .map(|item| item + 1)
            .filter(|item| *item > 1)
            .rev()
            .sum::<u32>();
        assert_eq!(cfg_test_helper(&items), total);
    }
}
