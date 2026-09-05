pub struct Production {
    name: String,
}

impl Production {
    pub fn name(&self) -> String {
        self.name.clone()
    }
}

#[cfg(test)]
mod tests {
    pub struct CfgTestFixture {
        label: String,
    }

    impl CfgTestFixture {
        pub fn label(&self) -> String {
            self.label.clone()
        }
    }

    #[test]
    fn test_function() {
        let fixture = CfgTestFixture {
            label: "x".to_owned(),
        };
        assert_eq!(fixture.label(), "x");
    }
}
