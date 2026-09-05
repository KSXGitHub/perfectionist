pub fn production(first: bool, second: bool, third: bool, fourth: bool, fifth: bool) -> bool {
    if first && second && third && fourth && fifth { true } else { false }
}

#[cfg(test)]
mod tests {
    pub fn cfg_test_helper(first: bool, second: bool, third: bool, fourth: bool, fifth: bool) -> bool {
        if first && second && third && fourth && fifth { true } else { false }
    }

    #[test]
    fn test_function() {
        let flag = true;
        if flag && flag && flag && flag && flag {
            assert!(cfg_test_helper(flag, flag, flag, flag, flag));
        }
    }
}
