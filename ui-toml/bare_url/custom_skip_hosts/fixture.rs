/// Good: `skip_hosts = ["rust-lang.org"]` suppresses this URL even
/// though it's a bare URL by default: https://rust-lang.org/ here.
fn _configured_skip_host() {}

/// Bad: `example.com` was a built-in default but the custom
/// `skip_hosts` replaces (not extends) the defaults, so it fires
/// again now: https://example.com/ here.
fn _former_default_now_fires() {}

fn main() {}
