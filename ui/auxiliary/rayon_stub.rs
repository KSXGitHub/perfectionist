// A stand-in for `rayon::iter::ParallelIterator`, carrying only the
// adapters the rule names. The names differ from the sequential set the
// way rayon's do: there is no `scan`, no `map_while` and no `rposition`;
// there is a `position_any` / `position_first` / `position_last` trio,
// declared on the `IndexedParallelIterator` subtrait as upstream declares
// them, a `find_map_any` / `find_map_first` / `find_map_last` trio in
// place of `find_map`, and a `find_first` / `find_any` pair in place of
// `find`.
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

        /// Here to be left alone: the item comes back out, so a
        /// leading `map` changes what this sees.
        fn filter<Body>(self, body: Body) -> Parallel<Self::Item>
        where
            Body: Fn(&Self::Item) -> bool + Send + Sync,
        {
            let mut kept = Vec::new();
            for item in self.into_items() {
                if body(&item) {
                    kept.push(item);
                }
            }
            Parallel(kept)
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

        fn find_map_any<Output: Send, Body>(self, body: Body) -> Option<Output>
        where
            Body: Fn(Self::Item) -> Option<Output> + Send + Sync,
        {
            self.find_map_first(body)
        }

        fn find_map_last<Output: Send, Body>(self, body: Body) -> Option<Output>
        where
            Body: Fn(Self::Item) -> Option<Output> + Send + Sync,
        {
            self.into_items().into_iter().filter_map(body).last()
        }

        /// Here to be left alone by the chain rule, which the item coming
        /// back out excludes, and reached by the predicate rule, whose
        /// `filter` takes the item the same way this does.
        fn find_first<Body>(self, body: Body) -> Option<Self::Item>
        where
            Body: Fn(&Self::Item) -> bool + Send + Sync,
        {
            self.into_items().into_iter().find(|item| body(item))
        }

        fn find_any<Body>(self, body: Body) -> Option<Self::Item>
        where
            Body: Fn(&Self::Item) -> bool + Send + Sync,
        {
            self.find_first(body)
        }

        fn try_fold<Accumulator: Send, Start, Body>(
            self,
            start: Start,
            body: Body,
        ) -> Parallel<Option<Accumulator>>
        where
            Start: Fn() -> Accumulator + Send + Sync,
            Body: Fn(Accumulator, Self::Item) -> Option<Accumulator> + Send + Sync,
        {
            let mut total = Some(start());
            for item in self.into_items() {
                total = total.and_then(|carried| body(carried, item));
            }
            Parallel(vec![total])
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

    /// The subtrait upstream declares the positional adapters on, which
    /// is why a lookup of the base trait alone never reaches them.
    pub trait IndexedParallelIterator: ParallelIterator {
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

        fn position_first<Body>(self, body: Body) -> Option<usize>
        where
            Body: Fn(Self::Item) -> bool + Send + Sync,
        {
            self.position_any(body)
        }

        fn position_last<Body>(self, body: Body) -> Option<usize>
        where
            Body: Fn(Self::Item) -> bool + Send + Sync,
        {
            let mut found = None;
            for (index, item) in self.into_items().into_iter().enumerate() {
                if body(item) {
                    found = Some(index);
                }
            }
            found
        }
    }

    impl<Item: Send> ParallelIterator for Parallel<Item> {
        type Item = Item;

        fn into_items(self) -> Vec<Item> {
            self.0
        }
    }

    impl<Item: Send> IndexedParallelIterator for Parallel<Item> {}
}

pub mod prelude {
    pub use super::iter::{IndexedParallelIterator, Parallel, ParallelIterator};
}
