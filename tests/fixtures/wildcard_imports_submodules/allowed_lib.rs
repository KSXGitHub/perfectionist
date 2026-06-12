#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, unused_imports, dead_code, reason = "fixture")]

#[allow(perfectionist::wildcard_imports, reason = "regression fixture")]
mod separate;
