use super::body_interior;
use text_block_macros::{text_block, text_block_fnl};

#[test]
fn braces_are_stripped() {
    assert_eq!(
        body_interior(text_block! {
            "{"
            "    work();"
            "}"
        }),
        text_block_fnl! {
            ""
            "    work();"
        },
    );
    assert_eq!(body_interior("work()"), "work()");
}
