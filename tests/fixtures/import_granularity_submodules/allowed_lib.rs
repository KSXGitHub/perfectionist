#![feature(register_tool)]
#![register_tool(perfectionist)]

#[allow(perfectionist::import_granularity, reason = "regression fixture")]
mod separate;
