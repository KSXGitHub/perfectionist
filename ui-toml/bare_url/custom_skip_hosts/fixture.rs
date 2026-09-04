/// Good: `skip_hosts = ["example.net"]` suppresses this URL even
/// though it's a bare URL by default: https://example.net/ here.
fn _configured_skip_host() {}

/// Bad: `example.com` isn't in the custom `skip_hosts`, so this bare
/// URL still fires — the custom list replaces, not extends, the
/// `localhost` default: https://example.com/ here.
fn _unlisted_host_fires() {}

fn main() {}
