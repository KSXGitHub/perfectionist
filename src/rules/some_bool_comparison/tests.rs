use super::{Rewrite, rewrite};

/// Every operator, every literal, and every state of an `Option<bool>`:
/// twelve comparisons, each run against the `unwrap_or` the rule would
/// put in its place. The suggestion is handed over as
/// `MachineApplicable`, so this is what that promise rests on.
///
/// The `Option<&bool>` payload needs no second table: comparing two
/// `&bool`s compares the referents, and the rewrite's `copied` reaches
/// the same `bool`s.
#[test]
fn the_rewrite_answers_as_the_comparison_did_in_every_state() {
    for equality in [true, false] {
        for literal in [true, false] {
            let Rewrite { default, negated } = rewrite(equality, literal);
            for state in [None, Some(true), Some(false)] {
                let compared = if equality {
                    state == Some(literal)
                } else {
                    state != Some(literal)
                };
                let rewritten = negated ^ state.unwrap_or(default);
                assert_eq!(
                    compared,
                    rewritten,
                    "`opt {} Some({literal})` and `{}opt.unwrap_or({default})` disagree on \
                     `{state:?}`",
                    if equality { "==" } else { "!=" },
                    if negated { "!" } else { "" },
                );
            }
        }
    }
}
