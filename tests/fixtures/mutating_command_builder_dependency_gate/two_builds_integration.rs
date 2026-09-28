use std::process::Command;

// Neither `#[test]` nor `cfg`-gated, so it reads as non-test code --
// which is what distinguishes the exemption from its naive form. A
// dev-dependency reaches it, so it has to be flagged.
fn helper() {
    let mut command = Command::new("ls");
    command.arg("integration-helper");
}

#[test]
fn body() {
    helper();
}
