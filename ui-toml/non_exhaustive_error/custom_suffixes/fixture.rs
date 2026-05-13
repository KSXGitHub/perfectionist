#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints)]
#![warn(perfectionist::non_exhaustive_error)]

// `extra_suffixes = ["Failure"]` adds to the built-in `["Error"]`
// list and `ignore_suffixes = ["Error"]` drops the default back
// out, so `Failure`-suffixed enums must fire and `Error`-suffixed
// enums must NOT fire purely by name. (`Error`-suffixed enums
// would still fire via the `impl Error` branch, but nothing here
// implements that trait.)

pub enum ConfigFailure {
    Variant,
}

pub enum RuntimeError {
    Variant,
}

fn main() {}
