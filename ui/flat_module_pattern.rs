// Loads `auxiliary/foo/mod.rs`, which the lint should flag.
#[path = "auxiliary/foo/mod.rs"]
mod foo;

// Loads `auxiliary/clean/clean.rs`, which is not a `mod.rs`
// and should NOT be flagged.
#[path = "auxiliary/clean/clean.rs"]
mod clean;

fn main() {
    foo::hi();
    clean::hi();
}
