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

// Bad: a `cfg`-gated derive is still ordered. The rule runs before
// expansion, so the wrapped `derive(...)` is reached regardless of
// whether the `cfg` predicate holds in this build.
#[cfg_attr(unix, derive(Debug, Clone, Copy))]
struct _CfgAttr;

// Bad: a `cfg_attr` may gate more than one attribute; every
// `derive(...)` after the predicate is ordered independently. Here only
// the first list is out of order (`Eq, PartialEq` is already sorted).
#[cfg_attr(unix, derive(Debug, Clone), derive(Eq, PartialEq))]
struct _CfgAttrMulti;

// Bad: a compound `cfg` predicate is left untouched while its gated
// `derive(...)` is ordered — the predicate is just the first item and
// is skipped regardless of its shape.
#[cfg_attr(all(unix, target_pointer_width = "64"), derive(Debug, Clone))]
struct _CfgAttrCompound;

// Bad: a `derive(...)` nested under several `cfg_attr` layers
// (`cfg_attr(a, cfg_attr(b, derive(...)))`, the same as
// `cfg_attr(all(a, b), derive(...))`) is reached by recursing into each
// gated `cfg_attr` argument list.
#[cfg_attr(unix, cfg_attr(target_os = "linux", derive(Debug, Clone)))]
struct _CfgAttrNested;

// Bad: a derive gated by a predicate that is FALSE on the test host is
// still ordered — `derive_ordering` is a pre-expansion pass and never
// evaluates the `cfg` predicate, so the check is host-independent. Keep
// the predicate as `windows` (false on the Unix CI host): "simplifying"
// it back to `unix` would silently drop the platform-independence
// coverage this case exists to pin.
#[cfg_attr(windows, derive(Debug, Clone, Copy))]
struct _CfgAttrInactivePredicate;

fn main() {}
