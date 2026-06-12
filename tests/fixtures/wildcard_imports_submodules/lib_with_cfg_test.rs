#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, unused_imports, dead_code, reason = "fixture")]

pub struct Thing;

mod separate;

#[cfg(test)]
mod tests {
    use super::*;
}
