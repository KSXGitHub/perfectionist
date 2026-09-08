pub fn production(a: bool, b: bool, c: bool) {
    if a {
        if b {
            if c {
                if a {
                    if b {
                        if c {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    pub fn cfg_test_helper(a: bool, b: bool, c: bool) {
        if a {
            if b {
                if c {
                    if a {
                        if b {
                            if c {}
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_function() {
        let a = true;
        if a {
            if a {
                if a {
                    if a {
                        if a {
                            if a {}
                        }
                    }
                }
            }
        }
        cfg_test_helper(a, a, a);
    }
}
