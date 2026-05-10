// Surfaces two pieces of build-time information to the launcher binaries:
//
// * The pinned nightly channel from `rust-toolchain`. The launcher uses it
//   to resolve a sysroot at runtime — first by looking for a matching
//   rustup toolchain, then by downloading from `static.rust-lang.org`.
// * The host triple. Distribution tarball URLs and rustup toolchain
//   directory names both embed it.

use std::env;
use std::fs;

fn main() {
    println!("cargo:rerun-if-changed=rust-toolchain");

    let raw = fs::read_to_string("rust-toolchain")
        .expect("failed to read `rust-toolchain` next to Cargo.toml");
    let channel = parse_channel(&raw).expect("could not extract `channel` from `rust-toolchain`");
    println!("cargo:rustc-env=PERFECTIONIST_PINNED_TOOLCHAIN={channel}");

    // `TARGET` is what we are compiling *for*. Our binaries are
    // platform-native, so this is also the host triple at runtime.
    let host = env::var("TARGET").expect("`TARGET` env from cargo");
    println!("cargo:rustc-env=PERFECTIONIST_HOST_TRIPLE={host}");
}

// Tiny TOML scanner — `rust-toolchain` only carries `[toolchain]` with a
// handful of keys, so a full TOML parser is unnecessary at build time.
fn parse_channel(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.split('#').next()?.trim();
        let Some(rest) = line.strip_prefix("channel") else {
            continue;
        };
        let rest = rest.trim_start();
        let rest = rest.strip_prefix('=')?.trim();
        let value = rest.trim_matches('"').trim_matches('\'');
        if !value.is_empty() {
            return Some(value.to_owned());
        }
    }
    None
}
