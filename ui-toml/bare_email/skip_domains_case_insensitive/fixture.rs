// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Good: `skip_domains = ["EXAMPLE.COM"]` suppresses this even though
/// the address is lowercase — domains match case-insensitively:
/// contact user@example.com about it.
fn _skipped_case_insensitive_domain() {}

/// Bad: a different domain isn't in `skip_domains`, so this bare
/// email still fires: other@elsewhere.test here.
fn _other_domain_fires() {}

fn main() {}
