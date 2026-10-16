// A stand-in for `pipe_trait::Pipe`. `impl<X> Pipe for X {}` and each
// method's body are upstream's own shape, which is why the split here is
// an identity rather than a measurement.
//
// Every method is here, because each is a table entry: `pipe` takes the
// receiver by value, and the other eight hand the closure a borrow, or a
// borrow of what the receiver converts to.
//
// The crate name is `pipe_trait` so that the path the rule looks up is
// the path the real trait has.

#![crate_name = "pipe_trait"]
#![allow(dead_code, unused, reason = "aux fixture")]

use std::borrow::{Borrow, BorrowMut};
use std::ops::{Deref, DerefMut};

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

    fn pipe_mut<'a, Output>(&'a mut self, body: impl FnOnce(&'a mut Self) -> Output) -> Output {
        body(self)
    }

    fn pipe_as_ref<'a, Param, Output>(&'a self, body: impl FnOnce(&'a Param) -> Output) -> Output
    where
        Self: AsRef<Param>,
        Param: ?Sized + 'a,
    {
        body(self.as_ref())
    }

    fn pipe_as_mut<'a, Param, Output>(
        &'a mut self,
        body: impl FnOnce(&'a mut Param) -> Output,
    ) -> Output
    where
        Self: AsMut<Param>,
        Param: ?Sized + 'a,
    {
        body(self.as_mut())
    }

    fn pipe_deref<'a, Param, Output>(&'a self, body: impl FnOnce(&'a Param) -> Output) -> Output
    where
        Self: Deref<Target = Param>,
        Param: ?Sized + 'a,
    {
        body(self)
    }

    fn pipe_deref_mut<'a, Param, Output>(
        &'a mut self,
        body: impl FnOnce(&'a mut Param) -> Output,
    ) -> Output
    where
        Self: DerefMut<Target = Param>,
        Param: ?Sized + 'a,
    {
        body(self)
    }

    fn pipe_borrow<'a, Param, Output>(&'a self, body: impl FnOnce(&'a Param) -> Output) -> Output
    where
        Self: Borrow<Param>,
        Param: ?Sized + 'a,
    {
        body(self.borrow())
    }

    fn pipe_borrow_mut<'a, Param, Output>(
        &'a mut self,
        body: impl FnOnce(&'a mut Param) -> Output,
    ) -> Output
    where
        Self: BorrowMut<Param>,
        Param: ?Sized + 'a,
    {
        body(self.borrow_mut())
    }
}

impl<Value> Pipe for Value {}
