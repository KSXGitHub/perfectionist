#![allow(dead_code, unused_imports)]

pub mod shared {
    pub struct S;
}

mod outer;

use std::time::Duration;
use crate::shared::S;
