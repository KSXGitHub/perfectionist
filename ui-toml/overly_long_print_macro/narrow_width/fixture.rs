// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// Fires under `max_line_width = 40`: this line is wider than 40
// columns but well within the default 100, so only the narrowed
// configuration flags it.
fn _medium_line() {
    let item = 1;
    println!("processing item number {item}\nfinished processing it now");
}

// Skipped even at width 40: a genuinely short line.
fn _tiny_line() {
    println!("a\nb");
}

fn main() {}
