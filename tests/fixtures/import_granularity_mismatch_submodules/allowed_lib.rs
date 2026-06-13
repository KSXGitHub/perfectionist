#![feature(register_tool)]
#![register_tool(perfectionist)]

#[allow(perfectionist::import_granularity_mismatch, reason = "regression fixture")]
mod separate;
