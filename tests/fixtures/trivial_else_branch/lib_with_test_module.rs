pub fn production(ready: bool) -> u32 {
    if ready {
        let first = 1;
        let second = first + 1;
        second + 1
    } else {
        return 0;
    }
}

#[cfg(test)]
mod tests {
    pub fn cfg_test_helper(ready: bool) -> u32 {
        if ready {
            let first = 1;
            let second = first + 1;
            second + 1
        } else {
            return 0;
        }
    }

    #[test]
    fn test_function() {
        let ready = true;
        if ready {
            let first = 1;
            let second = first + 1;
            assert_eq!(cfg_test_helper(ready), second + 1);
        } else {
            return;
        }
    }
}
