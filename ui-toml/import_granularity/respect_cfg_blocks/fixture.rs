#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Style is the default `module`, but `respect_cfg_blocks = false`, so a
// platform-gated import and an unconditional one from the same module
// fall into one group. Merging them would drop the `#[cfg]` gate, so the
// lint fires but withholds an autofix.

// `import_grouping` (active by default) is allowed here: under its
// `multi_block` style the cfg-gated import forms a trailing group, which
// this fixture places first to exercise granularity's cfg handling.
#[allow(perfectionist::import_grouping, reason = "exercises import_granularity")]
mod m {
    #[cfg(unix)]
    use std::collections::BTreeMap;
    use std::collections::HashMap;
}

fn main() {}
