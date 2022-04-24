#![feature(try_blocks)]
#![feature(result_flattening)]

extern crate core;

use std::fmt::Display;

use anyhow::{
    Context,
    Error,
    Result,
};
use tokio::task::JoinError;

mod exporter;
mod scraper;

pub mod cli;
pub mod model;

pub use self::{
    exporter::export,
    scraper::scrape,
};

pub fn task_context<T, C, F>(ctx: F) -> impl FnOnce(Result<Result<T>, JoinError>) -> Result<T>
where
    C: Display + Send + Sync + 'static,
    F: FnOnce() -> C,
{
    |res| res.map_err(Error::new).flatten().with_context(ctx)
}
