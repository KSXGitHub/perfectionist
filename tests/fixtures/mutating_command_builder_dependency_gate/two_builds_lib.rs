use std::process::Command;

pub fn production() {
    let mut command = Command::new("ls");
    command.arg("production-code");
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    #[test]
    fn inline() {
        let mut command = Command::new("ls");
        command.arg("inline-test-code");
    }
}
