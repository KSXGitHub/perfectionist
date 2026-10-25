//! A module with a print template long enough that
//! `perfectionist::overly_long_print_macro` folds it.

/// Print the fixture's diagnostic, whose template runs well past the
/// width the rule allows and carries an embedded newline to fold at.
pub fn announce(source: &str, command: &str) {
    println!("error: the operation failed because of {source} deep inside the subsystem\nhint: run {command} to repair it");
}

/// What this module contributes to the fixture's output.
pub fn report() -> String {
    announce("disk", "fsck");
    String::new()
}

#[cfg(test)]
mod tests {
    /// The call still runs, which folding its template cannot
    /// change. What it prints is held by the fixture's own `main`,
    /// whose output is compared either side of the rounds.
    #[test]
    fn the_announcement_still_runs() {
        assert_eq!(super::report(), "");
    }
}
