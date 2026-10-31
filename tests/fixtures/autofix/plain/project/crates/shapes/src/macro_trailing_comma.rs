//! A module whose macro invocations break the vertical trailing-comma
//! policy, which `perfectionist::macro_trailing_comma` restores.
//!
//! One case takes two rounds. Its invocation is stored across several
//! lines and would fit on one, so the round's rustfmt pass joins it,
//! and the comma that was the vertical form's becomes the
//! single-line form's gratuitous one. The fixer rewrites what the
//! check reported, which was before the formatter joined the lines,
//! so that comma is the next round's to remove.

/// The labels this module reports, one per line, with the comma left
/// off the last of them. The list cannot fit on one line, so the
/// formatter leaves the vertical layout, and the comma is the rule's
/// to add.
pub fn labels() -> Vec<&'static str> {
    vec![
        "the first label, long enough that the list cannot fit on one line",
        "the second label, long enough that the list cannot fit on one line",
        "the third label, long enough that the list cannot fit on one line"
    ]
}

/// How many labels there are, formatted across lines that would fit
/// on one.
pub fn summary() -> String {
    format!(
        "{} labels",
        labels().len(),
    )
}

/// What this module contributes to the fixture's output.
pub fn report() -> String {
    format!("macro_trailing_comma: {}\n", summary())
}

#[cfg(test)]
mod tests {
    /// Every label survives, which adding a trailing comma to the
    /// list cannot change.
    #[test]
    fn every_label_survives() {
        assert_eq!(super::labels().len(), 3);
        assert!(super::labels()[0].starts_with("the first label"));
    }

    /// The summary counts them all, which removing the comma after
    /// its last argument cannot change.
    #[test]
    fn the_summary_counts_every_label() {
        assert_eq!(super::summary(), "3 labels");
    }
}
