// A stand-in for `rayon::iter::ParallelIterator`, carrying only the
// adapters the rule names. The names differ from the sequential set the
// way rayon's do: there is no `scan`, no `map_while` and no `rposition`;
// there is `position_any`, and a `find_map_any` / `find_map_first` /
// `find_map_last` trio in place of `find_map`.
//
// `ParallelIterator: Sized + Send` and the `Send` bound on what `map`
// produces are upstream's, and they are the point: the rule declines a
// lifted step whose result cannot cross a thread.
//
// The receiver is a type of its own rather than an iterator, so that a
// call in a fixture resolves to one trait rather than two.
//
// The crate name is `rayon` so that the path the rule looks up is the
// path the real trait has.

#![crate_name = "rayon"]
#![allow(dead_code, unused, reason = "aux fixture")]

pub mod iter {
    /// A stand-in receiver, which no `Iterator` impl covers.
    pub struct Parallel<Item>(pub Vec<Item>);

    pub trait ParallelIterator: Sized + Send {
        type Item: Send;

        fn into_items(self) -> Vec<Self::Item>;

        fn map<Output: Send, Body>(self, body: Body) -> Parallel<Output>
        where
            Body: Fn(Self::Item) -> Output + Send + Sync,
        {
            let mut mapped = Vec::new();
            for item in self.into_items() {
                mapped.push(body(item));
            }
            Parallel(mapped)
        }

        fn for_each<Body>(self, body: Body)
        where
            Body: Fn(Self::Item) + Send + Sync,
        {
            for item in self.into_items() {
                body(item);
            }
        }

        fn any<Body>(self, body: Body) -> bool
        where
            Body: Fn(Self::Item) -> bool + Send + Sync,
        {
            for item in self.into_items() {
                if body(item) {
                    return true;
                }
            }
            false
        }

        fn all<Body>(self, body: Body) -> bool
        where
            Body: Fn(Self::Item) -> bool + Send + Sync,
        {
            !self.any(move |item| !body(item))
        }

        fn position_any<Body>(self, body: Body) -> Option<usize>
        where
            Body: Fn(Self::Item) -> bool + Send + Sync,
        {
            for (index, item) in self.into_items().into_iter().enumerate() {
                if body(item) {
                    return Some(index);
                }
            }
            None
        }

        fn find_map_first<Output: Send, Body>(self, body: Body) -> Option<Output>
        where
            Body: Fn(Self::Item) -> Option<Output> + Send + Sync,
        {
            for item in self.into_items() {
                if let Some(found) = body(item) {
                    return Some(found);
                }
            }
            None
        }

        fn fold<Accumulator: Send, Start, Body>(self, start: Start, body: Body) -> Parallel<Accumulator>
        where
            Start: Fn() -> Accumulator + Send + Sync,
            Body: Fn(Accumulator, Self::Item) -> Accumulator + Send + Sync,
        {
            let mut total = start();
            for item in self.into_items() {
                total = body(total, item);
            }
            Parallel(vec![total])
        }
    }

    impl<Item: Send> ParallelIterator for Parallel<Item> {
        type Item = Item;

        fn into_items(self) -> Vec<Item> {
            self.0
        }
    }
}

pub mod prelude {
    pub use super::iter::{Parallel, ParallelIterator};
}
