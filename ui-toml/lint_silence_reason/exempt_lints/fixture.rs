// `exempt_lints = ["clippy::module_name_repetitions"]`: an
// attribute whose every named lint is on the exempt list is
// accepted without a `reason`; a mixed list (one exempt, one not)
// still requires a `reason`.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]

// Good — every named lint is exempt.
#[allow(clippy::module_name_repetitions)]
fn fully_exempt() {}

// Bad — `dead_code` isn't exempt, so the rule still fires even
// though one of the two named lints is.
#[allow(clippy::module_name_repetitions, dead_code)]
fn mixed_with_non_exempt() {}

// Good — the exemption applies through `cfg_attr` too.
#[cfg_attr(all(), allow(clippy::module_name_repetitions))]
fn fully_exempt_cfg_attr() {}

fn main() {
    fully_exempt();
    mixed_with_non_exempt();
    fully_exempt_cfg_attr();
}
