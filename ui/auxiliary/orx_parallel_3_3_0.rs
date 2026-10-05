// A stand-in for `orx-parallel`'s `ParIter`, the trait 4.0 renamed to
// `Par`, carrying the methods that generation declared and the signatures
// upstream declared them with.
//
// It is here so that the rule is held to a version whose trait has another
// name, which declares `take_while` and `map_while` and no `fold`. Which
// adapter is which is read from the signatures rather than from the names,
// and those did not change across the rename, so the same reading answers
// both generations with no version to consult.
//
// The crate name is `orx_parallel` so that the crate the rule asks for is
// the crate the real trait is in.

#![crate_name = "orx_parallel"]
#![allow(dead_code, unused, reason = "aux fixture")]

/// A stand-in receiver, which no `Iterator` impl covers.
pub struct Parallel<Item>(pub Vec<Item>);

pub trait ParIter: Sized {
    type Item;

    fn into_items(self) -> Vec<Self::Item>;

    fn map<Out, H>(self, h: H) -> Parallel<Out>
    where
        H: Fn(Self::Item) -> Out + Copy + Send,
    {
        Parallel(self.into_items().into_iter().map(h).collect())
    }

    fn filter<H>(self, h: H) -> Parallel<Self::Item>
    where
        H: Fn(&Self::Item) -> bool + Copy + Send,
    {
        Parallel(self.into_items().into_iter().filter(|held| h(held)).collect())
    }

    fn filter_map<Out, H>(self, h: H) -> Parallel<Out>
    where
        H: Fn(Self::Item) -> Option<Out> + Copy + Send,
    {
        Parallel(self.into_items().into_iter().filter_map(h).collect())
    }

    fn flat_map<V, H>(self, h: H) -> Parallel<V::Item>
    where
        V: IntoIterator,
        H: Fn(Self::Item) -> V + Copy + Send,
    {
        let mut all = Vec::new();
        for item in self.into_items() {
            all.extend(h(item));
        }
        Parallel(all)
    }

    fn inspect<H>(self, h: H) -> Parallel<Self::Item>
    where
        H: Fn(&Self::Item) + Copy + Send,
    {
        let items = self.into_items();
        for item in &items {
            h(item);
        }
        Parallel(items)
    }

    fn for_each<F>(self, f: F)
    where
        F: Fn(Self::Item) + Send + Copy,
    {
        self.into_items().into_iter().for_each(f);
    }

    fn any<F>(self, f: F) -> bool
    where
        F: Fn(&Self::Item) -> bool + Sync,
    {
        self.into_items().iter().any(f)
    }

    fn all<F>(self, f: F) -> bool
    where
        F: Fn(&Self::Item) -> bool + Sync,
    {
        self.into_items().iter().all(f)
    }

    fn find<F>(self, f: F) -> Option<Self::Item>
    where
        F: Fn(&Self::Item) -> bool + Sync,
    {
        self.into_items().into_iter().find(|held| f(held))
    }


    fn take_while<W>(self, w: W) -> Parallel<Self::Item>
    where
        W: Fn(&Self::Item) -> bool + Sync + Clone,
    {
        Parallel(
            self.into_items()
                .into_iter()
                .take_while(|held| w(held))
                .collect(),
        )
    }

    fn map_while<Out, M>(self, m: M) -> Parallel<Out>
    where
        M: Fn(Self::Item) -> Option<Out> + Sync + Clone,
    {
        Parallel(self.into_items().into_iter().map_while(m).collect())
    }

    fn reduce<F>(self, f: F) -> Option<Self::Item>
    where
        F: Fn(Self::Item, Self::Item) -> Self::Item + Send + Copy,
    {
        self.into_items().into_iter().reduce(f)
    }
}

impl<Item> ParIter for Parallel<Item> {
    type Item = Item;

    fn into_items(self) -> Vec<Item> {
        self.0
    }
}
