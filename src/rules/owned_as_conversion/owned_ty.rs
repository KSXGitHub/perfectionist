//! Deciding, from a return type alone, whether a method hands the
//! caller something of their own.
//!
//! The question is *ownership*, not *allocation*. Whether a body
//! allocates is not decidable from a signature — `String::new()`
//! allocates nothing and `u32::to_string` allocates every time — so
//! the predicate here never claims it. What it claims is that the
//! caller was handed a value they must drop, where an `as_*` prefix
//! promised them a view.
//!
//! It is a deliberate under-approximation: a type counts as owned only
//! where this module can name it, so a third-party owning type is
//! missed rather than guessed at. That is also what leaves
//! `Cow<'_, str>` and the other borrow-capable wrappers alone, since
//! none of them is a type this module names.
//!
//! Only the outermost type is asked. A lifetime *inside* an owned
//! container does not make the container less owned: a
//! `Vec<Cow<'_, str>>` is still a `Vec` the caller drops.

use clippy_utils::ty::is_copy;
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::Symbol;

/// The `std` types that own a heap allocation whatever they are
/// parameterised with, keyed by `rustc_diagnostic_item`.
///
/// `Rc` and `Arc` are deliberately absent. From a signature there is
/// no telling a handle built here from one cloned out of a field, and
/// the second is a refcount bump the rule exists not to report.
const OWNING_TYPES: &[&str] = &[
    "Vec",
    "VecDeque",
    "LinkedList",
    "PathBuf",
    "OsString",
    // `CString` carries `cstring_type`; `field_copy` maps the same pair
    // in the other direction.
    "cstring_type",
    "HashMap",
    "HashSet",
    "BTreeMap",
    "BTreeSet",
    "BinaryHeap",
];

/// The wrappers that own nothing themselves, so their payload decides.
/// A `Result`'s error type is the conversion's failure path rather than
/// its product, so only the `Ok` type is read.
const TRANSPARENT_WRAPPERS: &[&str] = &["Option", "Result"];

/// Whether a caller handed this type receives something of their own,
/// where a borrow of the receiver would have been free.
pub(super) fn owned_return<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    // A shared reference is itself `Copy`, so a return type that is
    // already a borrow leaves here, along with every scalar and every
    // `Copy` ADT: handing one back by value is free, which is what the
    // prefix promises.
    if is_copy(cx, ty) {
        return false;
    }
    let (adt, args) = match ty.kind() {
        // One owning element is enough to make the whole aggregate the
        // caller's to drop.
        ty::Tuple(elements) => return elements.iter().any(|element| owned_return(cx, element)),
        ty::Array(element, _) => return owned_return(cx, *element),
        ty::Adt(adt, args) => (adt, args),
        _ => return false,
    };
    let did = adt.did();
    // `String` is a lang item (`#[lang = "String"]`) rather than a
    // diagnostic item, and so is `Box`.
    if Some(did) == cx.tcx.lang_items().string() || Some(did) == cx.tcx.lang_items().owned_box() {
        return true;
    }
    let is = |name: &str| cx.tcx.is_diagnostic_item(Symbol::intern(name), did);
    if TRANSPARENT_WRAPPERS.iter().any(|name| is(name)) {
        return args
            .types()
            .next()
            .is_some_and(|inner| owned_return(cx, inner));
    }
    OWNING_TYPES.iter().any(|name| is(name))
}
