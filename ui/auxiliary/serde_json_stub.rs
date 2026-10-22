// A stand-in for `serde_json`, carrying just enough of `json!` to
// build a value from a brace-delimited literal. The real macro expands
// to a tree of `Value` constructors; this one keeps the shape that
// matters to a caller, which is that `json!({ .. })` is written as a
// macro call and reads as a block of data.
//
// Nothing here is `perfectionist`-specific. It exists because a JSON
// literal is the everyday case of a default that is a macro call, and
// a fixture cannot depend on the crate itself.

/// What `json!` builds, reduced to the one variant a default needs.
pub enum Value {
    Null,
    Object(Vec<(String, String)>),
}

#[macro_export]
macro_rules! json {
    ({}) => {
        $crate::Value::Object(Vec::new())
    };
    ({ $($key:literal : $value:expr),* $(,)? }) => {
        $crate::Value::Object(vec![
            $(($key.to_owned(), $value.to_owned())),*
        ])
    };
    (null) => {
        $crate::Value::Null
    };
}
