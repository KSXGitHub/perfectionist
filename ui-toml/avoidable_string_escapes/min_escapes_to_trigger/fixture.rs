// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// `min_escapes_to_trigger = 2`: a literal with only one
// eliminable escape stays untouched; a literal with two or more
// is flagged.

fn _below_threshold() {
    // One `\\` escape — must NOT fire.
    let _ = "C:\\Users";
}

fn _at_threshold() {
    // Three `\\` escapes — must fire.
    let _ = "C:\\Users\\foo\\bar";
}

fn main() {
    _below_threshold();
    _at_threshold();
}
