#![allow(dead_code, unused_variables, reason = "ui fixture")]

use std::sync::atomic::AtomicUsize;

// Bad: free `static` item with a single-letter name.
static N: AtomicUsize = AtomicUsize::new(0);

// Bad: free `static` item with a different single-letter name.
static D: u64 = 64;

// Good: descriptive `static` item name.
static REQUEST_COUNT: AtomicUsize = AtomicUsize::new(0);

fn run() {
    // Bad: block-level `static` item with a single-letter name.
    static A: u32 = 1;
    let _ = A;
}

fn main() {
    run();
}
