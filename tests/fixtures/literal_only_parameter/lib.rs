pub fn exported(value: u32, verbose: bool) -> u32 {
    if verbose { value + 1 } else { value }
}

fn private(value: u32, verbose: bool) -> u32 {
    if verbose { value + 1 } else { value }
}

pub fn calls() -> u32 {
    exported(1, true) + private(1, true) + private(2, false)
}

#[cfg(test)]
mod tests {
    fn cfg_test_helper(value: u32, verbose: bool) -> u32 {
        if verbose { value + 1 } else { value }
    }

    #[test]
    fn test_function() {
        assert_eq!(cfg_test_helper(1, true), 2);
        assert_eq!(super::private(1, false), 1);
    }
}
