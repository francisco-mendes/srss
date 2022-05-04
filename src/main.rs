#![feature(try_blocks)]

use anyhow::Result;
use clap::Parser;
use futures_util::FutureExt;
use srss::cli::{
    CliArgs,
    Credentials,
    DriverArgs,
    ExportArgs,
};
use tokio::sync::mpsc as tokio_mpsc;
use tracing::instrument;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    try {
        let CliArgs {
            driver,
            credentials,
            output,
            month,
            log_filter,
        } = CliArgs::parse();

        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::builder().parse_lossy(log_filter))
            .compact()
            .init();

        tracing::trace!("logging initialized");

        let credentials = Credentials::form_args_or_prompt(credentials)?;

        tracing::trace!("credentials acquired");

        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(run(driver, month, credentials, output))?;
    }
}

#[instrument(skip_all, name = "main")]
async fn run(
    driver: DriverArgs,
    month: Option<String>,
    credentials: Credentials,
    output: ExportArgs,
) -> Result<()> {
    try {
        let (report_sx, report_rx) = tokio_mpsc::channel(20);

        let scraper = tokio::spawn(srss::scrape(driver, month, credentials, report_sx))
            .map(srss::task_context("unable to scrape dashboard"));
        let reports = tokio::spawn(srss::export(output, report_rx))
            .map(srss::task_context("unable to write report logs"));

        tokio::try_join!(scraper, reports)?;
        tracing::info!("done");
    }
}
