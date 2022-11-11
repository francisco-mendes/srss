#![feature(try_blocks)]
#![feature(result_flattening)]
#![feature(control_flow_enum)]

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

pub fn task_context<T, C>(ctx: C) -> impl FnOnce(Result<Result<T>, JoinError>) -> Result<T>
where
    C: Display + Send + Sync + 'static,
{
    |res| res.map_err(Error::new).flatten().context(ctx)
}
