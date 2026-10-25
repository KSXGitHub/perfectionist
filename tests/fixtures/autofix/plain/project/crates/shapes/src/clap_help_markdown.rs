//! A module whose clap-derived documentation carries a code span,
//! which reaches the terminal verbatim and which
//! `perfectionist::clap_help_markdown` strips.

/// The fixture's command line.
#[derive(clap::Parser)]
pub struct Cli {
    /// Path to the `manifest` file.
    pub manifest: String,
}

/// What this module contributes to the fixture's output.
pub fn report() -> String {
    let cli = Cli {
        manifest: "fixture.toml".to_owned(),
    };
    format!("clap_help_markdown: {}\n", cli.manifest)
}

#[cfg(test)]
mod tests {
    /// The field still carries what it was given, which stripping a
    /// code span from its help text cannot change.
    #[test]
    fn the_field_carries_its_value() {
        assert_eq!(super::report(), "clap_help_markdown: fixture.toml\n");
    }
}
