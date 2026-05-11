use std::rc::Rc;
use std::sync::Arc;

fn _bad_arc() {
    let value: Arc<Vec<u8>> = Arc::new(vec![1, 2, 3]);
    // Direct method-call form on an `Arc<T>` binding: should fire.
    let _ = value.clone();
}

fn _bad_rc() {
    let value: Rc<String> = Rc::new(String::new());
    // Direct method-call form on an `Rc<T>` binding: should fire.
    let _ = value.clone();
}

fn _bad_field() {
    struct State {
        inner: Arc<u32>,
    }
    let state = State { inner: Arc::new(1) };
    // Field access as the receiver: the snippet for the suggestion
    // is `state.inner`, which doesn't need parens around the `&`.
    let _ = state.inner.clone();
}

fn _bad_through_reference(value: &Arc<u32>) {
    // Receiver is `&Arc<T>`. Method resolution still picks
    // `<Arc<T> as Clone>::clone` (whose self type is `&Arc<T>` and
    // matches the receiver type at the very first probe step), so
    // `value.clone()` already returns `Arc<T>`. The autofix
    // `Arc::clone(&value)` preserves both the type and behaviour.
    let _ = value.clone();
}

fn _good_qualified() {
    let value: Arc<u32> = Arc::new(1);
    // Bare qualified form — desired shape, not flagged.
    let _ = Arc::clone(&value);
    // Turbofish-typed qualified form — also accepted.
    let _ = Arc::<u32>::clone(&value);
    // UFCS form — also accepted.
    let _ = <Arc<u32> as Clone>::clone(&value);

    let value: Rc<u32> = Rc::new(1);
    let _ = Rc::clone(&value);
    let _ = Rc::<u32>::clone(&value);
    let _ = <Rc<u32> as Clone>::clone(&value);
}

fn _good_other_types() {
    // `.clone()` on non-`Arc`/`Rc` types is out of scope.
    let owned_string: String = String::from("hello");
    let _ = owned_string.clone();
    let owned_vec: Vec<u8> = vec![1, 2, 3];
    let _ = owned_vec.clone();
    let boxed: Box<u32> = Box::new(1);
    let _ = boxed.clone();
}

fn main() {}
