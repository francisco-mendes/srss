use std::{
    self,
    iter,
    process::Stdio,
    time::Duration,
};

use anyhow::{
    Context,
    Result,
};
use regex::Regex;
use thirtyfour::{
    prelude::*,
    stringmatch::StringMatch,
};
use tokio::{
    process::{
        Child,
        Command,
    },
    sync::mpsc::Sender,
};
use tracing::instrument;

use crate::{
    cli::{
        Credentials,
        DriverArgs,
    },
    model::{
        Record,
        Report,
        Station,
    },
};

#[instrument(skip_all)]
pub async fn scrape(
    args: DriverArgs,
    month: Option<String>,
    credentials: Credentials,
    report_sink: Sender<Report>,
) -> Result<()> {
    try {
        assert_driver_compatible(&args).await?;

        let mut process = spawn_webdriver(&args).await?;

        let mut settings = DesiredCapabilities::chrome();
        settings.set_headless()?;
        let mut driver = WebDriver::new(&format!("http://localhost:{}/srss", args.port), settings)
            .await
            .context("unable to create webdriver")?;
        driver.set_window_rect(0, 0, 1920, 1080).await?;
        tracing::debug!("webdriver initialized");

        scrape_inner(&mut driver, month, credentials, report_sink).await?;

        driver.quit().await.context("unable to close driver")?;
        process.try_wait().context("chromedriver still running")?;
        tracing::trace!("webdriver closed");
    }
}

async fn assert_driver_compatible(args: &DriverArgs) -> Result<()> {
    let major_version_regex = Regex::new(r"\d+").unwrap();
    try {
        let driver_version = Command::new(&args.executable)
            .arg("--version")
            .output()
            .await
            .context("unable to query webdriver version")?
            .stdout;
        tracing::trace!("webdriver version fetched");

        let driver_version = String::from_utf8_lossy(&driver_version);
        let driver_version = driver_version
            .matches(&major_version_regex)
            .next()
            .context("webdriver version not found")?;
        tracing::trace!(version.major = %driver_version);

        let browser_version = Command::new("reg")
            .args([
                "query",
                r"HKEY_CURRENT_USER\Software\Google\Chrome\BLBeacon",
                "-v",
                "Version",
            ])
            .output()
            .await
            .context("unable to query browser version")?
            .stdout;
        tracing::trace!("browser version fetched");

        let browser_version = String::from_utf8_lossy(&browser_version);
        let browser_version = browser_version
            .matches(&major_version_regex)
            .next()
            .context("browser version not found")?;
        tracing::trace!(version = %browser_version);

        anyhow::ensure!(
            driver_version == browser_version,
            "incompatible web driver and browser versions: {} != {}",
            driver_version,
            browser_version
        );
    }
}

async fn spawn_webdriver(args: &DriverArgs) -> Result<Child> {
    try {
        tracing::trace!("killing any previous webdriver process");
        let killer = Command::new("taskkill")
            .args(["-f", "-im"])
            .arg(&args.executable)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("unable to kill previous webdriver process")?
            .wait()
            .await
            .context("error while waiting to kill previous webdriver process")?;

        if !matches!(killer.code(), Some(0 | 128)) {
            anyhow::bail!("failed to kill previous webdriver process");
        }

        tracing::trace!("spawning new webdriver process");

        let process = Command::new(&args.executable)
            .args([&format!("--port={}", args.port), "--url-base", "srss"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .kill_on_drop(true)
            .spawn()
            .context("unable to spawn webdriver process")?;

        tracing::debug!(port = args.port, "webdriver process spawned");
        tracing::warn!(
            process = process.id(),
            "if program fails, you may have to kill webdriver process and browser manually"
        );
        process
    }
}

async fn scrape_inner(
    driver: &mut WebDriver,
    month: Option<String>,
    credentials: Credentials,
    report_sink: Sender<Report>,
) -> Result<()> {
    try {
        login_to_dashboard(driver, &credentials)
            .await
            .context("unable to login")?;
        let mut stations = list_stations(driver)
            .await
            .context("unable to list power stations")?;

        stations.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        for station in stations {
            let mut counter = 10;
            loop {
                let err = match export_report(driver, month.as_deref(), &station).await {
                    Ok(records) => {
                        report_sink.send(Report { station, records }).await?;
                        break;
                    }
                    Err(err) => err,
                };
                tracing::error!(%station.id, %station.name, "failed to extract report");
                if counter == 0 {
                    anyhow::bail!(err);
                } else {
                    eprintln!("Error: {:?}", err);
                    counter -= 1;
                }
            }
        }
    }
}

#[instrument(skip_all)]
async fn login_to_dashboard(driver: &mut WebDriver, credentials: &Credentials) -> Result<()> {
    try {
        tracing::trace!("entering login page");
        driver.goto(include_str!("loginpage.secret.txt")).await?;

        tracing::debug!("searching for login form");
        let user_input = driver
            .query(By::Id("username"))
            .and_clickable()
            .single()
            .await
            .context("unable to find user input")?;
        let pass_input = driver
            .query(By::Id("value"))
            .and_clickable()
            .single()
            .await
            .context("unable to find password input")?;
        let login_btn = driver
            .query(By::Id("submitDataverify"))
            .and_clickable()
            .single()
            .await
            .context("unable to submit button")?;

        tracing::debug!(user = %credentials.username, "attempting to login");
        driver
            .action_chain()
            .send_keys_to_element(&user_input, &credentials.username)
            .send_keys_to_element(&pass_input, &credentials.password)
            .click_element(&login_btn)
            .perform()
            .await
            .context("unable to perform login")?;

        tracing::info!(user = %credentials.username, "logged in");
    }
}

#[instrument(skip_all)]
async fn list_stations(driver: &mut WebDriver) -> Result<Vec<Station>> {
    let href_match = Regex::new(include_str!("stationlink.secret.txt")).unwrap();
    try {
        let mut stations = Vec::with_capacity(128);

        tracing::trace!("waiting for site to load");
        let _: Result<_> = try {
            driver
                .query(By::Id("login_info_win_close"))
                .and_clickable()
                .single()
                .await?
                .click()
                .await?;
        };
        tracing::trace!("closed login toast");

        loop {
            tracing::trace!("searching for stations in page");

            let station_link_path = By::XPath(r"// tr / td[3] // a");
            let links = driver.query(station_link_path).all_required().await?;

            tracing::trace!("processing stations");
            for link in &links {
                let name = link.text().await?;

                let href = link
                    .attr("href")
                    .await
                    .context("missing link in station name")?
                    .context("link is not a string")?;

                let id = href_match
                    .captures(&href)
                    .and_then(|cap| cap.name("id"))
                    .map(|cap| String::from(cap.as_str()))
                    .context("missing station id in link")?;

                tracing::trace!(%id, %name, "found station");
                stations.push(Station { id, name });
            }
            tracing::debug!(
                count = links.len(),
                total = stations.len(),
                "stations processed"
            );

            let next = driver
                .query(By::ClassName("ant-pagination-next"))
                .single()
                .await?;

            if next
                .class_name()
                .await?
                .unwrap_or_default()
                .contains("ant-pagination-disabled")
            {
                break;
            }

            tracing::debug!("advancing to next page");
            next.click().await?;
            wait_for_table_reload(driver).await?;
        }

        tracing::info!(total = stations.len(), "all stations found");
        stations
    }
}

#[instrument(skip_all, fields(%station.id, %station.name))]
async fn export_report(
    driver: &mut WebDriver,
    month: Option<&str>,
    station: &Station,
) -> Result<Vec<Record>> {
    try {
        tracing::debug!("accessing reports");
        let station_report_url = format!(include_str!("reportpage.secret.txt"), station.id);
        driver
            .goto(station_report_url)
            .await
            .context("failed to go to report page")?;
        driver
            .refresh()
            .await
            .context("failed to refresh report page")?;

        tracing::debug!("setting time granularity");
        let granularity_selector_path = By::Css(".ant-select-selection-item[title='Daily']");
        driver
            .query(granularity_selector_path)
            .single()
            .await
            .map_err(|_| anyhow::anyhow!("time granularity dropdown not found"))?
            .click()
            .await
            .map_err(|_| anyhow::anyhow!("unable to click on time granularity dropdown"))?;

        tracing::trace!("picking monthly granularity");
        driver
            .action_chain()
            .send_keys(Key::Down + Key::Enter)
            .perform()
            .await
            .map_err(|_| anyhow::anyhow!("unable to select monthly time granularity"))?;

        tracing::debug!("setting page size");
        let pagination_path = By::Css(".ant-select-selection-item[title='10 / page']");
        driver
            .query(pagination_path)
            .single()
            .await
            .map_err(|_| anyhow::anyhow!("pagination dropdown not found"))?
            .click()
            .await
            .map_err(|_| anyhow::anyhow!("failed to click on pagination dropdown"))?;

        tracing::trace!("picking page size");
        driver
            .action_chain()
            .send_keys(Key::Up + Key::Up + Key::Enter)
            .perform()
            .await
            .map_err(|_| anyhow::anyhow!("unable to select max page size"))?;

        if let Some(month) = month {
            let time = driver
                .query(By::Id("statisticTime"))
                .single()
                .await
                .map_err(|_| anyhow::anyhow!("unable to find period setter"))?;

            driver
                .action_chain()
                .click_element(&time)
                .key_down(Key::Control)
                .send_keys("a")
                .key_up(Key::Control)
                .send_keys(Key::Backspace + month + Key::Enter)
                .perform()
                .await
                .map_err(|_| anyhow::anyhow!("unable to set the month"))?;
        }
        wait_for_table_reload(driver)
            .await
            .context("unable to wait for table to reload")?;

        tracing::trace!("scanning for dates");
        let dates_path = By::XPath(
            r#"// tbody[@class="ant-table-tbody"] / tr[contains(@class, 'ant-table-row')] / td[1]"#,
        );
        let dates = driver
            .query(dates_path)
            .all()
            .await
            .map_err(|_| anyhow::anyhow!("unable to find dates"))?;

        tracing::trace!("scanning for yields");
        let yields_path = By::XPath(
            r#"// tbody[@class="ant-table-tbody"] / tr[contains(@class, 'ant-table-row')] / td[2]"#,
        );
        let yields = driver
            .query(yields_path)
            .all()
            .await
            .map_err(|_| anyhow::anyhow!("unable to find yields"))?;

        anyhow::ensure!(
            dates.len() == yields.len(),
            "malformed report table: found {} date cells and {} yield cells",
            dates.len(),
            yields.len()
        );
        tracing::trace!(count = dates.len(), "found records");

        let mut data = Vec::with_capacity(dates.len());

        for (date, pv_yield) in iter::zip(dates, yields) {
            tracing::trace!("processing record");

            let date = date
                .text()
                .await
                .map_err(|_| anyhow::anyhow!("date is invalid"))?;
            let pv_yield = pv_yield
                .text()
                .await
                .map_err(|_| anyhow::anyhow!("yield is invalid"))?
                .parse()
                .ok();

            if let Some(month) = month {
                anyhow::ensure!(
                    date.contains(month),
                    "got data for the wrong month: {} in not in {}",
                    date,
                    month
                );
            }

            tracing::trace!(%date, "yield" = ?pv_yield, "added record");
            data.push(Record { date, pv_yield })
        }

        tracing::debug!("station scraped");
        data
    }
}

async fn wait_for_table_reload(driver: &mut WebDriver) -> Result<()> {
    try {
        tracing::trace!("waiting for table reload");
        driver
            .query(By::ClassName("ant-spin-container"))
            .single()
            .await?
            .wait_until()
            .lacks_class(StringMatch::new("ant-spin-blur").partial())
            .await?
    }
}
