//! A module folding a singular command setter over a sequence, which
//! `perfectionist::folded_command_setter` replaces with the plural
//! setter the same trait offers.

use command_extra::CommandExtra;
use std::process::Command;

/// A command with each name in `names` removed from its environment.
pub fn without_each(names: &[&str]) -> Command {
    let start = Command::new("ls");
    names.iter().fold(start, CommandExtra::without_env)
}

/// What this module contributes to the fixture's output.
pub fn report() -> String {
    let removed = without_each(&["ALPHA", "BETA"]).get_envs().count();
    format!("folded_command_setter: {removed}\n")
}

#[cfg(test)]
mod tests {
    /// Every name is still removed, and removed by name, which
    /// replacing the fold with the plural setter cannot change.
    #[test]
    fn every_name_is_still_removed() {
        let command = super::without_each(&["ALPHA", "BETA"]);
        let names: Vec<_> = command.get_envs().map(|(name, _)| name).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&std::ffi::OsStr::new("ALPHA")));
        assert!(names.contains(&std::ffi::OsStr::new("BETA")));
    }
}
