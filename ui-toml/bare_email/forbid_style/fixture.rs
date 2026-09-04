// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Bad: bare email under `style = "forbid"` should emit no
/// suggestion at all — only prose telling the author to move the
/// address out of the source — security@example.net.
fn _bare_email_forbid_style() {}

fn main() {}
