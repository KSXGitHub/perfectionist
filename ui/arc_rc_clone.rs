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
    // `value.clone()` already returns `Arc<T>`. The autofix passes
    // `value` straight through — no extra `&`, because the receiver
    // is already a reference.
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

fn _bad_chained(arcs: &[Arc<u32>]) {
    // Chained receiver: `arcs.first().unwrap()` is `&Arc<u32>`, so
    // the rewrite passes it straight through without an extra `&`.
    // `Sugg` for a method call is `NonParen`, so no wrapping parens
    // either.
    let _ = arcs.first().unwrap().clone();
}

#[allow(suspicious_double_ref_op)]
fn _good_double_reference(value: &&Arc<u32>) {
    // Receiver is `&&Arc<T>`. Method probe finds the blanket
    // `<&Arc<T> as Clone>::clone` (self type `&&Arc<T>`) at the
    // first candidate level, so `value.clone()` returns
    // `&Arc<T>` — a reference copy, not a refcount bump. The
    // result-type filter keeps the lint silent rather than rewrite
    // into something that would change the expression's type.
    // (`suspicious_double_ref_op` flags this for unrelated reasons;
    // silenced here so the fixture documents only our rule.)
    let _ = value.clone();
}

fn _good_weak() {
    // `Weak<T>` carries the diagnostic name `ArcWeak` / `RcWeak`,
    // not `Arc` / `Rc`. The rule deliberately scopes to the strong
    // handles only — `weak.clone()` is a refcount bump on the
    // weak count and is left alone.
    let strong = Arc::new(1);
    let weak: std::sync::Weak<u32> = Arc::downgrade(&strong);
    let _ = weak.clone();

    let rc_strong = Rc::new(1);
    let rc_weak: std::rc::Weak<u32> = Rc::downgrade(&rc_strong);
    let _ = rc_weak.clone();
}

fn main() {}
