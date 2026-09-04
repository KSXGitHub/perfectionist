// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
macro_rules! my_macro {
    ($($item:expr),* $(,)?) => {{ $(let _ = $item;)* 0 }};
}

// Bad: multi-line `vec!` missing the trailing comma.
fn _multi_line_missing() {
    let _ = vec![
        1,
        2,
        3
    ];
}

// Bad: single-line `vec!` with a gratuitous trailing comma.
fn _single_line_extra() {
    let _ = vec![1, 2, 3,];
}

// Bad: multi-line single-argument `dbg!` missing the trailing comma.
fn _single_argument_multi_line() {
    let _ = dbg!(
        compute()
    );
}

// Bad: multi-line `assert_eq!` with a panic-message tail and no
// trailing comma.
fn _assert_eq_multi_line() {
    assert_eq!(
        actual(),
        expected(),
        "decoder mismatch"
    );
}

// Bad: qualified path resolves to a curated macro via final-segment
// matching.
fn _qualified_path() {
    let _ = std::vec![
        1,
        2
    ];
}

// Good: multi-line invocation already has a trailing comma.
fn _ok_multi_line() {
    let _ = vec![
        1,
        2,
        3,
    ];
}

// Good: single-line invocation already lacks a trailing comma.
fn _ok_single_line() {
    let _ = vec![1, 2, 3];
}

// Skipped: `vec![value; count]` is the repeat form (top-level `;`).
fn _skip_repeat_form() {
    let _ = vec![0; 4];
}

// Skipped: empty body — nothing to add or remove.
fn _skip_empty() {
    let _: Vec<i32> = vec![];
    let _: &str = concat!();
}

// Skipped: an uncurated macro is not eligible.
fn _skip_uncurated() {
    let _ = my_macro!(
        1,
        2,
        3
    );
}

// Skipped: the first top-level token shares its line with the opening
// delimiter — the visual-indent shape rustfmt produces, and rustfmt's
// `trailing_comma = "Vertical"` does not add a trailing comma to that
// shape (and actively strips any that gets added). The two tools have
// to agree, so the lint skips it. The first three cases are single
// multi-line elements; the fourth is a multi-element list whose first
// element happens to start on the opening-delimiter line, locking in
// that the predicate keys off the first token's line, not the element
// count.
fn _compact_first_token_skipped() {
    let _ = vec![format!(
        "{}",
        42
    )];
    let _ = vec![Inner {
        name: 1,
        kids: 2,
    }];
    let _ = wrap(vec![bar(
        "very long name",
        4069,
    )]);
    let _ = vec![1, 2,
        3, 4];
}

struct Inner {
    name: i32,
    kids: i32,
}

fn bar(_: &str, _: i32) -> Inner {
    Inner { name: 0, kids: 0 }
}

fn wrap<Value>(value: Value) -> Value {
    value
}

fn compute() -> i32 {
    0
}

fn actual() -> i32 {
    0
}

fn expected() -> i32 {
    0
}

fn main() {}
