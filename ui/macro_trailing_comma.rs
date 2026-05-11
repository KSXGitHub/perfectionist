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
