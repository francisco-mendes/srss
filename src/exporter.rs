use std::path::PathBuf;

use anyhow::{
    Context,
    Error,
    Result,
};
use futures_util::{
    StreamExt,
    TryStreamExt,
};
use tokio::{
    fs,
    fs::File,
    io::AsyncWriteExt,
    sync::mpsc::Receiver,
};
use tokio_stream::wrappers::ReceiverStream;
use tracing::instrument;

use crate::{
    cli::{
        ExportArgs,
        ExportFormat,
    },
    model::{
        Record,
        Report,
    },
};

fn to_line<F>(format: F) -> impl Fn(&Record) -> String + Clone + Send + Sync + 'static
where
    F: Fn(&Record) -> String + Clone + Send + Sync + 'static,
{
    move |record| format(record) + "\n"
}

pub async fn export(out: ExportArgs, rx: Receiver<Report>) -> Result<()> {
    try {
        match out.format {
            ExportFormat::Values => {
                export_to_file(out.destination, "txt", rx, to_line(Record::to_value)).await?
            }
            ExportFormat::Log => {
                export_to_file(out.destination, "log", rx, to_line(Record::to_string)).await?
            }
            ExportFormat::Csv => {
                export_to_file(out.destination, "csv", rx, to_line(Record::to_csv)).await?
            }
        }
    }
}

#[instrument(skip_all, name = "export", fields(kind = %extension, dir = %directory.display()))]
async fn export_to_file<F>(
    directory: PathBuf,
    extension: &str,
    rx: Receiver<Report>,
    format: F,
) -> Result<()>
where
    F: Fn(&Record) -> String + Clone + Sync + Send + 'static,
{
    try {
        let _ = fs::remove_dir_all(&directory).await;
        fs::create_dir_all(&directory)
            .await
            .context("Unable to create output directory")?;
        tracing::debug!("created output directory");

        ReceiverStream::new(rx)
            .map(|report| {
                (
                    directory
                        .join(&report.station.name)
                        .with_extension(extension),
                    report,
                )
            })
            .then(|(dest, report)| tokio::spawn(write_report(report, dest, format.clone())))
            .map(|out| out.map_err(Error::new).flatten())
            .try_collect::<()>()
            .await
            .context("error writing station reports")?;
        tracing::debug!("all station reports written");
    }
}

#[instrument(skip_all, fields(station.id = %report.station.id, station.name = %report.station.name))]
async fn write_report<F>(report: Report, file: PathBuf, format: F) -> Result<()>
where
    F: Fn(&Record) -> String,
{
    try {
        tracing::trace!("creating output file");
        let mut file = File::create(file).await.with_context(|| {
            format!(
                "failed to create output file for station {}",
                report.station.name
            )
        })?;

        tracing::trace!("writing report");
        for record in &report.records {
            file.write_all(format(record).as_bytes())
                .await
                .with_context(|| {
                    format!(
                        "failed to output record for station {}",
                        report.station.name
                    )
                })?;
        }
        tracing::info!(records.count = report.records.len(), "report written");
    }
}
