// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Bad: bare email under `style = "both"` should emit a single
/// `MachineApplicable` suggestion that both wraps the address in
/// angle brackets and prefixes it with `mailto:` —
/// security@example.net.
fn _bare_email_both_style() {}

fn main() {}
