use command_extra::CommandExtra;
use std::process::Command;

pub fn reaches_both() {
    let _ = middle::touch(Command::new("ls"));
    let mut command = Command::new("ls");
    command.envs([("LANG", "C")]);
}
