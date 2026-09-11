// edition:2024
#![feature(register_tool, stmt_expr_attributes)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn map_and_filter() {
    let _ = [1, 2].into_iter().map(|value| {
        let doubled = value * 2;
        doubled + 1
    }).filter(|value| {
        let positive = *value > 0;
        positive
    });
}

fn visit_and_short_circuit() -> Result<(), ()> {
    [1, 2].into_iter().for_each(|value| {
        dbg!(value);
        dbg!(value + 1);
    });
    [1, 2].into_iter().try_for_each(|value| {
        dbg!(value);
        Ok(())
    })
}

fn fold_and_scan() {
    let _ = [1, 2].into_iter().fold(0, |total, value| {
        let doubled = value * 2;
        total + doubled
    });
    let _ = [1, 2].into_iter().scan(0, |total, value| {
        *total += value;
        Some(*total)
    });
}

fn qualified_and_generic<Values: Iterator<Item = i32>>(values: Values) {
    let _ = Iterator::map(values, |value| {
        let doubled = value * 2;
        doubled
    });
}

fn async_callback() {
    let _ = [1, 2].into_iter().map(async |value| {
        let doubled = value * 2;
        doubled
    });
}

fn single_expressions() {
    let _ = [1, 2].into_iter().map(|value| match value {
        0 => 1,
        _ => value,
    });
    let _ = [1, 2].into_iter().map(|value| {
        if value > 0 { value } else { 0 }
    });
    [1, 2].into_iter().for_each(|value| { dbg!(value); });
    [1, 2].into_iter().for_each(|_| {});
    let _ = [1, 2].into_iter().map(|value| { ;;; value ;;; });
    let _ = [1, 2].into_iter().map(|value| { { let doubled = value * 2; doubled } });
}

fn named_callback(value: i32) -> i32 {
    let doubled = value * 2;
    doubled
}

fn named_callbacks_are_not_inline() {
    let callback = |value| { let doubled = value * 2; doubled };
    let _ = [1, 2].into_iter().map(callback);
    let _ = [1, 2].into_iter().map(named_callback);
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
    [1, 2].into_iter().for_each(|_| { multiple_statements!(); });
    [1, 2].into_iter().for_each(|_| {
        multiple_statements!();
        dbg!(3);
    });
    [1, 2].into_iter().for_each(|_| {
        empty_statements!();
        dbg!(3);
    });
    let _ = [1, 2].into_iter().map(|value| vec![value, value]);
    let _ = [1, 2].into_iter().map(generated_closure!());
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
    Custom.map(|value| { let doubled = value * 2; doubled });
    Parallel.map(|value| { let doubled = value * 2; doubled });
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
    <Overridden as Iterator>::for_each(Overridden, |value| {
        let doubled = value * 2;
        dbg!(doubled);
    });
}

fn initial_accumulator_is_not_callback() {
    let _ = std::iter::empty::<i32>().fold(
        || { dbg!(1); dbg!(2); },
        |accumulator, _| accumulator,
    );
}

fn closure_expectation_is_honored() {
    let _ = [1, 2].into_iter().map(
        #[expect(perfectionist::multi_statement_iterator_closure, reason = "test closure scope")]
        |value| { let doubled = value * 2; doubled },
    );
}

#[path = "auxiliary/callbacks.rs"]
mod callbacks;

fn main() {}
