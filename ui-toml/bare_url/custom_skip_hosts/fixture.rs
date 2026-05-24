/// Good: `skip_hosts = ["example.net"]` suppresses this URL even
/// though it's a bare URL by default: https://example.net/ here.
fn _configured_skip_host() {}

/// Bad: `example.com` was a built-in default but the custom
/// `skip_hosts` replaces (not extends) the defaults, so it fires
/// again now: https://example.com/ here.
fn _former_default_now_fires() {}

fn main() {}
