#![allow(unused_imports, unused)]
#![feature(register_tool)]
#![register_tool(perfectionist)]

// Control: a crate-root foldable pair that is NOT under the `#[allow]`,
// so the rule must still flag it. This proves the rule is active in this
// project — without it, the `#[allow]` assertion below would pass
// trivially even if separate-file submodules were skipped entirely.
use std::fmt;
use std::fmt::Display;

#[allow(perfectionist::combined_self_import, reason = "regression fixture")]
mod separate;
