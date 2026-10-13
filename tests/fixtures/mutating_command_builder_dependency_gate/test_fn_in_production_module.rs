use std::process::Command;

pub fn production() {
    let mut command = Command::new("ls");
    command.arg("production-code");
}

// Test-exclusive as a node, but its module is the library's own root.
// `#[test]` implies `#[cfg(test)]`, so the shipping build never sees
// this function -- yet the import the remedy asks for would land at the
// top of this file, which that build does compile.
#[test]
fn top_level_test_fn() {
    let mut command = Command::new("ls");
    command.arg("test-fn-in-production-module");
}
