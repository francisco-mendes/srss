use std::path::PathBuf;

use clap::{
    ArgEnum,
    Args,
    Parser,
};
use tracing::instrument;

/// Solar Report Scraping Software
///
/// SRSS scrapes data from a web dashboard containing telemetry for solar power installations and
/// stores it into the excel file for later use.
#[derive(Parser)]
pub struct CliArgs {
    #[clap(flatten)]
    pub driver: DriverArgs,
    #[clap(flatten)]
    pub credentials: CredentialArgs,
    #[clap(flatten)]
    pub output: ExportArgs,
}

#[derive(Args)]
pub struct CredentialArgs {
    /// Username/Email to login
    #[clap(long = "user")]
    pub username: Option<String>,
    /// Password to login
    #[clap(long = "pass")]
    pub password: Option<String>,
}

#[derive(Args)]
pub struct DriverArgs {
    /// Path to the web driver executable
    #[clap(short, long = "exe", default_value = "./chromedriver.exe")]
    pub executable: PathBuf,
    /// Port to run the driver at
    #[clap(short, long, default_value = "4444")]
    pub port: u16,
}

#[derive(Args)]
pub struct ExportArgs {
    /// How to export the data
    #[clap(arg_enum, short = 't', long = "type", default_value = "values")]
    pub export: ExportType,
    #[clap(short, long = "dest", default_value = "report/")]
    pub destination: PathBuf,
}

#[derive(ArgEnum, Eq, PartialEq, Copy, Clone, Debug)]
pub enum ExportType {
    #[clap(name = "values")]
    Values,
    #[clap(name = "log")]
    Log,
    #[clap(name = "csv")]
    Csv,
    #[clap(name = "edit")]
    Edit,
}

#[derive(Args)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

impl Credentials {
    #[instrument(skip_all)]
    pub fn form_args_or_prompt(args: CredentialArgs) -> anyhow::Result<Self> {
        try {
            let username = match args.username {
                Some(user) => user,
                None => dialoguer::Input::new()
                    .with_prompt("Username/Email")
                    .interact()?,
            }
            .trim()
            .to_string();

            let password = match args.password {
                Some(pass) => pass,
                None => dialoguer::Password::new()
                    .with_prompt("Password")
                    .interact()?,
            }
            .trim()
            .to_string();
            Self { username, password }
        }
    }
}
