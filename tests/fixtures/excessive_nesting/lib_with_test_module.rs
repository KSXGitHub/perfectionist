pub fn production(first: bool, second: bool, third: bool, fourth: bool) {
    if first {
        if second {
            if third {
                if fourth {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    pub fn cfg_test_helper(first: bool, second: bool, third: bool, fourth: bool) {
        if first {
            if second {
                if third {
                    if fourth {}
                }
            }
        }
    }

    #[test]
    fn test_function() {
        let ready = true;
        if ready {
            if ready {
                if ready {
                    if ready {}
                }
            }
        }
        cfg_test_helper(ready, ready, ready, ready);
    }
}
