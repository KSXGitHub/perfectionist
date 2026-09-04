#![allow(dead_code, unused_variables, reason = "ui fixture")]

// Bad: free `const` item with a single-letter name.
const N: usize = 2;

// Bad: free `const` item with a different single-letter name.
const D: u64 = 64;

// Good: descriptive `const` item name.
const DIMENSION_COUNT: usize = 2;

struct Buffer {
    bytes: [u8; 64],
}

impl Buffer {
    // Bad: associated `const` in an inherent impl with a single-letter name.
    const C: usize = 64;

    // Good: descriptive associated `const`.
    const DEFAULT_CAPACITY: usize = 64;
}

trait HasCapacity {
    // Bad: associated `const` declaration in a `trait` with a single-letter name.
    const M: usize;

    // Bad: trait `const` declaration with a default value.
    const K: usize = 8;
}

impl HasCapacity for Buffer {
    // Bad: associated `const` in a trait impl with a single-letter name.
    const M: usize = 16;
    const K: usize = 8;
}

fn run() {
    // Bad: block-level `const` item with a single-letter name.
    const A: u32 = 1;
    let _ = A;
}

fn main() {
    run();
}
