// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![allow(dead_code, unused_variables, reason = "ui fixture")]

// Bad: const generic parameter with a single letter.
struct Data<Left, Right, const M: usize, const N: usize> {
    left: [Left; M],
    right: [Right; N],
}

// Bad: const generic on a free function.
fn first_n<const K: usize>(slice: &[u8]) -> &[u8] {
    &slice[..K]
}

// Bad: const generic on an `impl` block.
struct Buf<const C: usize>([u8; C]);

impl<const C: usize> Buf<C> {
    fn new() -> Self {
        Self([0; C])
    }
}

// Good: descriptive const generic name.
struct Matrix<const ROWS: usize, const COLS: usize> {
    cells: [[f64; COLS]; ROWS],
}

// Good: lifetime parameters are not const generics.
fn borrow<'a>(text: &'a str) -> &'a str {
    text
}

fn main() {}
