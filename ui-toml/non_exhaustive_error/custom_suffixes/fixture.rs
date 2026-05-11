// `suffixes = ["Failure"]` replaces the default `["Error"]` list,
// so `Failure`-suffixed enums must fire and `Error`-suffixed enums
// must NOT fire purely by name. (`Error`-suffixed enums would still
// fire via the `impl Error` branch, but nothing here implements that
// trait.)

pub enum ConfigFailure {
    Variant,
}

pub enum RuntimeError {
    Variant,
}

fn main() {}
