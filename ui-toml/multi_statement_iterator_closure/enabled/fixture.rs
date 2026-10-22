// edition:2024
#![feature(iter_intersperse, register_tool, stmt_expr_attributes, try_blocks)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn map_and_filter() {
    // Bad: both callbacks in the chain hold a `let` and a tail expression.
    let _ = [1, 2].into_iter().map(|value| {
        let doubled = value * 2;
        doubled + 1
    }).filter(|value| {
        let positive = *value > 0;
        positive
    });
}

fn visit_and_short_circuit() -> Result<(), ()> {
    // Bad: a visiting callback holding two `dbg!` statements.
    [1, 2].into_iter().for_each(|value| {
        dbg!(value);
        dbg!(value + 1);
    });
    // Bad: a short-circuiting callback holding a `dbg!` and a tail expression.
    [1, 2].into_iter().try_for_each(|value| {
        dbg!(value);
        Ok(())
    })
}

fn fold_and_scan() {
    // Bad: a reducing callback holding a `let` and a tail expression.
    let _ = [1, 2].into_iter().fold(0, |total, value| {
        let doubled = value * 2;
        total + doubled
    });
    // Bad: a generating callback holding an assignment and a tail expression.
    let _ = [1, 2].into_iter().scan(0, |total, value| {
        *total += value;
        Some(*total)
    });
}

fn qualified_and_generic<Values: Iterator<Item = i32>>(values: Values) {
    // Bad: a qualified call's callback holding a `let` and a tail expression.
    let _ = Iterator::map(values, |value| {
        let doubled = value * 2;
        doubled
    });
}

fn async_callback() {
    // Bad: an async callback holding a `let` and a tail expression.
    let _ = [1, 2].into_iter().map(async |value| {
        let doubled = value * 2;
        doubled
    });
}

fn single_expressions() {
    // Not flagged: one `match` expression, however many arms it has.
    let _ = [1, 2].into_iter().map(|value| match value {
        0 => 1,
        _ => value,
    });
    // Not flagged: one `if` expression inside a block.
    let _ = [1, 2].into_iter().map(|value| {
        if value > 0 { value } else { 0 }
    });
    // Not flagged: one statement, the `dbg!` call.
    [1, 2].into_iter().for_each(|value| { dbg!(value); });
    // Not flagged: an empty body counts nothing.
    [1, 2].into_iter().for_each(|_| {});
    // Not flagged: empty semicolons do not count.
    let _ = [1, 2].into_iter().map(|value| { ;;; value ;;; });
    // Not flagged: a nested block is one expression.
    let _ = [1, 2].into_iter().map(|value| { { let doubled = value * 2; doubled } });
}

fn named_callback(value: i32) -> i32 {
    let doubled = value * 2;
    doubled
}

fn named_callbacks_are_not_inline() {
    let callback = |value| { let doubled = value * 2; doubled };
    // Not flagged: a `let`-bound closure passed by name is not inline.
    let _ = [1, 2].into_iter().map(callback);
    // Not flagged: a function passed by name is not inline.
    let _ = [1, 2].into_iter().map(named_callback);
}

fn immediately_invoked_closure() {
    // Not flagged: a closure the source does not pass to a covered method.
    (|value| {
        let doubled = value * 2;
        doubled
    })(1);
}

macro_rules! multiple_statements {
    () => { dbg!(1); dbg!(2); };
}
macro_rules! empty_statements {
    () => {};
}
macro_rules! generated_closure {
    () => { |value| { let doubled = value * 2; doubled } };
}
macro_rules! evaluate {
    ($value:expr) => { $value };
}

#[expect(perfectionist::impure_macro_arguments, reason = "exercise a callback inside macro arguments")]
fn macro_boundaries() {
    // Not flagged: a macro invocation counts as one statement.
    [1, 2].into_iter().for_each(|_| { multiple_statements!(); });
    // Bad: the macro invocation and the `dbg!` beside it count two.
    [1, 2].into_iter().for_each(|_| {
        multiple_statements!();
        dbg!(3);
    });
    // Bad: a macro expanding to nothing still counts as one statement.
    [1, 2].into_iter().for_each(|_| {
        empty_statements!();
        dbg!(3);
    });
    // Not flagged: one expression, the `vec!` call.
    let _ = [1, 2].into_iter().map(|value| vec![value, value]);
    // Not flagged: a macro generated the closure.
    let _ = [1, 2].into_iter().map(generated_closure!());
    // Not flagged: a callback inside a macro invocation's arguments.
    let _ = evaluate!([1, 2].into_iter().map(|value| { let doubled = value * 2; doubled }));
}

struct Custom;
impl Custom {
    fn map(&self, callback: impl Fn(i32) -> i32) { callback(1); }
}
trait ParallelIterator {
    fn map(&self, callback: impl Fn(i32) -> i32) { callback(1); }
}
struct Parallel;
impl ParallelIterator for Parallel {}
trait Stream {
    fn map(&self, callback: impl Fn(i32) -> i32) { callback(1); }
}
struct AsyncStream;
impl Stream for AsyncStream {}

fn non_iterator_methods() {
    // Not flagged: an inherent `map` on a plain struct.
    Custom.map(|value| { let doubled = value * 2; doubled });
    // Not flagged: a parallel iterator's own `map`.
    Parallel.map(|value| { let doubled = value * 2; doubled });
    // Not flagged: an asynchronous stream's own `map`.
    AsyncStream.map(|value| { let doubled = value * 2; doubled });
}

struct Overridden;
impl Iterator for Overridden {
    type Item = i32;
    fn next(&mut self) -> Option<i32> { None }
    fn for_each<Callback>(self, mut callback: Callback)
    where Callback: FnMut(i32) {
        callback(1);
    }
}

fn overridden_iterator_method() {
    // Bad: an override of `Iterator::for_each` is still an `Iterator` method.
    <Overridden as Iterator>::for_each(Overridden, |value| {
        let doubled = value * 2;
        dbg!(doubled);
    });
    // Bad: method-call syntax resolves to the impl's own `for_each`, which
    // only reaches `Iterator` by way of the trait item it overrides.
    Overridden.for_each(|value| {
        let doubled = value * 2;
        dbg!(doubled);
    });
}

fn initial_accumulator_is_not_callback() {
    // Not flagged: the two-statement closure is the accumulator, not the callback.
    let _ = std::iter::empty::<i32>().fold(
        || { dbg!(1); dbg!(2); },
        |accumulator, _| accumulator,
    );
    // Bad: the callback beside the accumulator holds a `let` and a tail expression.
    let _ = std::iter::empty::<i32>().fold(
        || { dbg!(3); dbg!(4); },
        |accumulator, _| {
            let kept = accumulator;
            kept
        },
    );
    // Not flagged: the expectation sits on the callback, not on the accumulator.
    let _ = std::iter::empty::<i32>().fold(
        || { dbg!(5); dbg!(6); },
        #[expect(
            perfectionist::multi_statement_iterator_closure,
            reason = "the expectation belongs to the callback, not to the closure beside it"
        )]
        |accumulator, _| {
            let kept = accumulator;
            kept
        },
    );
}

fn closure_expectation_is_honored() {
    // Not flagged: an expectation on the callback suppresses the report.
    let _ = [1, 2].into_iter().map(
        #[expect(perfectionist::multi_statement_iterator_closure, reason = "test closure scope")]
        |value| { let doubled = value * 2; doubled },
    );
}

fn parenthesised_callbacks() {
    // Bad: parentheses around the callback.
    let _ = [1, 2].into_iter().map((|value| {
        let doubled = value * 2;
        doubled
    }));
    // Bad: two pairs of parentheses around the callback.
    let _ = [1, 2].into_iter().map(((|value| {
        let doubled = value * 2;
        doubled
    })));
}

fn wrapped_block_bodies(values: Vec<i32>) -> Vec<Result<i32, ()>> {
    // Bad: an `unsafe` block body is counted through.
    let _ = [1, 2].into_iter().map(|value| unsafe {
        let doubled = value * 2;
        doubled
    });
    // Bad: a labelled block body is counted through.
    let _ = [1, 2].into_iter().map(|value| 'doubling: {
        let doubled = value * 2;
        break 'doubling doubled;
    });
    // Bad: an `async move` block body is counted through.
    let _ = [1, 2].into_iter().map(|value| async move {
        let doubled = value * 2;
        doubled
    });
    // Bad: a `const` block body is counted through.
    let _ = [1, 2].into_iter().map(|_| const {
        let doubled = 2 * 2;
        doubled
    });
    // Bad: a `try` block body is counted through.
    values
        .into_iter()
        .map(|value| try {
            let doubled = value * 2;
            doubled
        })
        .collect()
}

fn parenthesised_bodies() {
    // Bad: parentheses around the block body.
    let _ = [1, 2].into_iter().map(|value| ({
        let doubled = value * 2;
        doubled
    }));
    // Bad: parentheses around an `unsafe` block body.
    let _ = [1, 2].into_iter().map(|value| ((unsafe {
        let doubled = value * 2;
        doubled
    })));
}

fn generator_callback(values: Vec<i32>) {
    // Bad: `intersperse_with`'s callback holds a `let` and a tail expression.
    let _ = values.into_iter().intersperse_with(|| {
        let separator = 0;
        separator
    });
}

fn sortedness_predicates(values: Vec<i32>) {
    // Bad: `is_sorted_by`'s predicate holds a `let` and a tail expression.
    let _ = values.iter().is_sorted_by(|left, right| {
        let ordered = left <= right;
        ordered
    });
    // Bad: `is_sorted_by_key`'s callback holds a `let` and a tail expression.
    let _ = values.iter().is_sorted_by_key(|value| {
        let key = *value;
        key
    });
}

fn item_and_cfg_statements() {
    // Bad: an item declaration counts as a statement.
    let _ = [1, 2].into_iter().map(|value| {
        struct Marker;
        value
    });
    // Bad: a statement a `cfg` removes still counts as written.
    let _ = [1, 2].into_iter().map(|value| {
        #[cfg(any())]
        let value = value + 1;
        value
    });
}

#[path = "auxiliary/callbacks.rs"]
mod callbacks;

fn main() {}
