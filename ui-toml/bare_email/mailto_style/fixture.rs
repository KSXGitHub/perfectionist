/// Bad: bare email under `style = "mailto"` should emit a single
/// `MachineApplicable` suggestion prefixing `mailto:` —
/// security@example.net.
fn _bare_email_mailto_style() {}

fn main() {}
