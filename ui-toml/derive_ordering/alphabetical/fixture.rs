// Bad: `Debug` should come after `Clone` and `Copy` under
// alphabetical ordering.
#[derive(Debug, Clone, Copy)]
struct _Out;

// Good: already in ASCII-case-insensitive alphabetical order.
#[derive(Clone, Copy, Debug)]
struct _Ok;

// Good: a single derive is trivially sorted.
#[derive(Debug)]
struct _Single;

// Bad: `Hash` < `PartialEq` lexically, so the entries here are out
// of order.
#[derive(PartialEq, Eq, Hash)]
struct _MixedCase;

// Bad and multi-line. The autofix would flatten the list onto one
// line, so the applicability is downgraded to `MaybeIncorrect`.
// `MaybeIncorrect` is not visible in the rendered diagnostic text
// (the `help: reorder the derive list: ...` label still appears);
// its user-visible effect is that `cargo fix` will not auto-apply
// the suggestion, leaving the multi-line layout intact.
#[derive(
    Debug,
    Clone,
    Copy,
)]
struct _MultiLine;

// Bad and contains an inline block comment between entries. The
// derive fits on one line, but the autofix would silently drop the
// `/* keep me */`, so the applicability is downgraded to
// `MaybeIncorrect` for the same reason as the multi-line case.
#[derive(Debug /* keep me */, Clone, Copy)]
struct _InlineComment;

fn main() {}
