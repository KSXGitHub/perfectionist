// A stand-in for `rayon::iter::ParallelIterator`, carrying only the
// adapters the rule names. The names differ from the sequential set the
// way rayon's do: there is no `scan`, no `map_while` and no `rposition`;
// there is a `flat_map_iter` beside `flat_map`, for flattening what can
// only be walked in sequence;
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

    /// A stand-in for upstream's trait of the same name, which is what
    /// tells `flat_map` from `flat_map_iter`: one flattens what can be
    /// walked in parallel and the other what can be walked in sequence.
    pub trait IntoParallelIterator {
        type Item: Send;

        fn into_par_iter(self) -> Parallel<Self::Item>;
    }

    impl<Item: Send> IntoParallelIterator for Parallel<Item> {
        type Item = Item;

        fn into_par_iter(self) -> Parallel<Item> {
            self
        }
    }

    impl<Item: Send> IntoParallelIterator for Vec<Item> {
        type Item = Item;

        fn into_par_iter(self) -> Parallel<Item> {
            Parallel(self)
        }
    }

    /// A stand-in for the `either` crate's type of that name, which
    /// upstream's `partition_map` answers with.
    pub enum Either<Left, Right> {
        Left(Left),
        Right(Right),
    }

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

        fn filter_map<Output: Send, Body>(self, body: Body) -> Parallel<Output>
        where
            Body: Fn(Self::Item) -> Option<Output> + Send + Sync,
        {
            let mut kept = Vec::new();
            for item in self.into_items() {
                if let Some(made) = body(item) {
                    kept.push(made);
                }
            }
            Parallel(kept)
        }

        fn flat_map<Output: Send, Made, Body>(self, body: Body) -> Parallel<Output>
        where
            Body: Fn(Self::Item) -> Made + Send + Sync,
            Made: IntoParallelIterator<Item = Output>,
        {
            let mut made = Vec::new();
            for item in self.into_items() {
                made.extend(body(item).into_par_iter().into_items());
            }
            Parallel(made)
        }

        fn flat_map_iter<Output: Send, Made, Body>(self, body: Body) -> Parallel<Output>
        where
            Body: Fn(Self::Item) -> Made + Send + Sync,
            Made: IntoIterator<Item = Output>,
        {
            let mut made = Vec::new();
            for item in self.into_items() {
                made.extend(body(item));
            }
            Parallel(made)
        }

        fn for_each<Body>(self, body: Body)
        where
            Body: Fn(Self::Item) + Send + Sync,
        {
            for item in self.into_items() {
                body(item);
            }
        }

        /// Upstream bounds what the closure answers with on `Try`, which
        /// is unstable, so this one takes a `Result`. The rule reads how
        /// the item arrives rather than what comes back.
        fn try_for_each<Error, Body>(self, body: Body) -> Result<(), Error>
        where
            Body: Fn(Self::Item) -> Result<(), Error> + Send + Sync,
        {
            for item in self.into_items() {
                body(item)?;
            }
            Ok(())
        }

        fn partition_map<Left, Right, Body>(self, body: Body) -> (Vec<Left>, Vec<Right>)
        where
            Body: Fn(Self::Item) -> Either<Left, Right> + Send + Sync,
            Left: Send,
            Right: Send,
        {
            let (mut left, mut right) = (Vec::new(), Vec::new());
            for item in self.into_items() {
                match body(item) {
                    Either::Left(made) => left.push(made),
                    Either::Right(made) => right.push(made),
                }
            }
            (left, right)
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
    pub use super::iter::{
        Either, IndexedParallelIterator, IntoParallelIterator, Parallel, ParallelIterator,
    };
}
