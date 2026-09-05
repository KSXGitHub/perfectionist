use super::body_interior;

#[test]
fn braces_are_stripped() {
    assert_eq!(body_interior("{\n    work();\n}"), "\n    work();\n");
    assert_eq!(body_interior("work()"), "work()");
}
