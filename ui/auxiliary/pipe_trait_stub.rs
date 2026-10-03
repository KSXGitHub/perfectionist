// A stand-in for `pipe_trait::Pipe`. `impl<X> Pipe for X {}` and
// `pipe` defined as `body(self)` are upstream's own shape, which is why
// the split here is an identity rather than a measurement.
//
// `pipe_ref` is here to be left alone: it hands the closure a borrow, so
// a leading `pipe` would be handing it something else.
//
// The crate name is `pipe_trait` so that the path the rule looks up is
// the path the real trait has.

#![crate_name = "pipe_trait"]
#![allow(dead_code, unused, reason = "aux fixture")]

pub trait Pipe {
    fn pipe<Output>(self, body: impl FnOnce(Self) -> Output) -> Output
    where
        Self: Sized,
    {
        body(self)
    }

    fn pipe_ref<'a, Output>(&'a self, body: impl FnOnce(&'a Self) -> Output) -> Output {
        body(self)
    }
}

impl<Value> Pipe for Value {}
