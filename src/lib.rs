#![allow(clippy::needless_range_loop)]
#![allow(clippy::neg_cmp_op_on_partial_ord)]
pub(crate) use std::cmp::Ordering as CmpOrd;
pub(crate) use std::collections::BinaryHeap;
pub(crate) use std::fs;
pub(crate) use std::io::{BufWriter, Read as IoRead, Write as IoWrite};
pub(crate) use std::rc::Rc;
pub(crate) use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
pub(crate) use std::sync::{Mutex, OnceLock};
pub(crate) use std::time::Instant;

mod config;
pub(crate) use config::*;

mod expr;
pub(crate) use expr::*;

mod math;
pub(crate) use math::*;

mod operators;
pub(crate) use operators::*;

mod frontier;
pub(crate) use frontier::*;

mod budget;
pub(crate) use budget::*;

mod model;
pub(crate) use model::*;

mod fit;
pub(crate) use fit::*;

mod generators;
pub(crate) use generators::*;

mod harvest;
pub(crate) use harvest::*;

mod search;
pub(crate) use search::*;

mod output;
pub(crate) use output::*;

mod driver;
pub(crate) use driver::split_row;
pub use driver::{run, Config};

pub fn fuzz_parse(s: &str) {
    let _ = expr::Parser::parse(s);
}

pub fn fuzz_split_row(line: &str, delim: char) {
    let _ = split_row(line, delim);
}

pub fn fuzz_ingest(data: &str) {
    let _ = driver::ingest(data, "target");
}

#[cfg(test)]
mod tests;
