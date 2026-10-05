//! The page's search and filter affordances: the magnifier and the two
//! funnels, the inert `<template>`s their markup lives in, the scripts
//! that clone and drive them, and the stylesheet rules that paint them.
//!
//! Split by what each kind of test reads rather than by which affordance
//! it covers, which is also where the seams fall: the markup tests share
//! a `<template>` reader, the stylesheet tests share a rule reader, and
//! the script tests need neither. Splitting by affordance instead would
//! have left both halves wanting the same readers.
//!
//! The fixtures and the stylesheet lookup these share with the rest of
//! the page's tests stay in the parent module.

mod markup;
mod scripts;
mod style;
