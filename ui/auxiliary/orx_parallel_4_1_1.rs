// A stand-in for `orx-parallel`'s `Par`, the trait 4.0 renamed from
// `ParIter`, carrying the methods the rule reads and the signatures
// upstream declares them with.
//
// Which adapter is which is read from these signatures rather than from
// their names, so the shapes are the point: `map` and `for_each` are handed
// the item, `filter`, `any`, `all` and `find` are lent it, and `fold` takes
// state first so its item is the second parameter. `take_while` and
// `map_while` are absent as they are upstream from 4.0, which is what a
// companion fixture against the older trait is for.
//
// The crate name is `orx_parallel` so that the crate the rule asks for is
// the crate the real trait is in.

#![crate_name = "orx_parallel"]
#![allow(dead_code, unused, reason = "aux fixture")]

/// A stand-in receiver, which no `Iterator` impl covers.
pub struct Parallel<Item>(pub Vec<Item>);

pub trait Par: Sized {
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

    fn fold<B, I, F>(self, init: I, f: F) -> Vec<B>
    where
        I: Fn() -> B + Sync,
        F: Fn(&mut B, Self::Item) + Copy + Send,
    {
        let mut accumulator = init();
        for item in self.into_items() {
            f(&mut accumulator, item);
        }
        vec![accumulator]
    }

    fn reduce<F>(self, f: F) -> Option<Self::Item>
    where
        F: Fn(Self::Item, Self::Item) -> Self::Item + Send + Copy,
    {
        self.into_items().into_iter().reduce(f)
    }
}

impl<Item> Par for Parallel<Item> {
    type Item = Item;

    fn into_items(self) -> Vec<Item> {
        self.0
    }
}
