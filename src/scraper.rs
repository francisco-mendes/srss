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
    query::StringMatch,
    OptionRect,
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
    credentials: Credentials,
    report_sink: Sender<Report>,
) -> Result<()> {
    try {
        assert_driver_compatible(&args).await?;

        let mut process = spawn_webdriver(&args).await?;

        let mut settings = DesiredCapabilities::chrome();
        settings.set_headless()?;
        let mut driver = WebDriver::new_with_timeout(
            &format!("http://localhost:{}/srss", args.port),
            settings,
            Some(Duration::from_secs(60)),
        )
        .await
        .with_context(|| "unable to create webdriver")?;
        driver
            .set_window_rect(OptionRect::new().with_size(1920, 1080))
            .await?;
        tracing::debug!("webdriver initialized");

        scrape_inner(&mut driver, credentials, report_sink).await?;

        driver
            .quit()
            .await
            .with_context(|| "unable to close driver")?;
        process
            .try_wait()
            .with_context(|| "chromedriver still running")?;
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
            .with_context(|| "unable to query webdriver version")?
            .stdout;
        tracing::trace!("webdriver version fetched");

        let driver_version = String::from_utf8_lossy(&driver_version);
        let driver_version = driver_version
            .matches(&major_version_regex)
            .next()
            .with_context(|| "webdriver version not found")?;
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
            .with_context(|| "unable to query browser version")?
            .stdout;
        tracing::trace!("browser version fetched");

        let browser_version = String::from_utf8_lossy(&browser_version);
        let browser_version = browser_version
            .matches(&major_version_regex)
            .next()
            .with_context(|| "webdriver version not found")?;
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
            .with_context(|| "unable to kill previous webdriver process")?
            .wait()
            .await
            .with_context(|| "error while waiting to kill previous webdriver process")?;

        if !matches!(killer.code(), Some(0 | 128)) {
            anyhow::bail!("failed to kill previous webdriver process");
        }

        tracing::trace!("spawning new webdriver process");

        let process = Command::new(&args.executable)
            .args([&format!("--port={}", args.port), "--url-base=srss"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .kill_on_drop(true)
            .spawn()
            .with_context(|| "unable to spawn webdriver process")?;

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
    credentials: Credentials,
    report_sink: Sender<Report>,
) -> Result<()> {
    try {
        login_to_dashboard(driver, &credentials)
            .await
            .with_context(|| "unable to login")?;
        let stations = list_stations(driver)
            .await
            .with_context(|| "unable to list power stations")?;

        for station in stations {
            match export_report(driver, &station).await {
                Ok(records) => report_sink.send(Report { station, records }).await?,
                Err(err) => {
                    tracing::error!(%station.id, %station.name, "unable to extract report, skipping station");
                    eprintln!("Error: {:?}", err);
                }
            };
        }
    }
}

#[instrument(skip_all)]
async fn login_to_dashboard(driver: &mut WebDriver, credentials: &Credentials) -> Result<()> {
    const LOGIN_PAGE: &str = include_str!("loginpage.secret.txt");
    const USER_INPUT_SELECTOR: By = By::Id("username");
    const PASS_INPUT_SELECTOR: By = By::Id("value");
    const LOGIN_BUTTON_SELECTOR: By = By::Id("submitDataverify");
    try {
        tracing::trace!("entering login page");
        driver.get(LOGIN_PAGE).await?;

        tracing::debug!("searching for login form");
        let user_input = driver
            .query(USER_INPUT_SELECTOR)
            .and_clickable()
            .single()
            .await
            .with_context(|| "unable to find user input")?;
        let pass_input = driver
            .query(PASS_INPUT_SELECTOR)
            .and_clickable()
            .single()
            .await
            .with_context(|| "unable to find password input")?;
        let login_btn = driver
            .query(LOGIN_BUTTON_SELECTOR)
            .and_clickable()
            .single()
            .await
            .with_context(|| "unable to submit button")?;

        tracing::debug!(user = %credentials.username, "attempting to login");
        driver
            .action_chain()
            .send_keys_to_element(&user_input, &credentials.username)
            .send_keys_to_element(&pass_input, &credentials.password)
            .click_element(&login_btn)
            .perform()
            .await
            .with_context(|| "unable to perform login")?;

        tracing::info!(user = %credentials.username, "logged in");
    }
}

#[instrument(skip_all)]
async fn list_stations(driver: &mut WebDriver) -> anyhow::Result<Vec<Station>> {
    const CLOSE_TOAST_SELECTOR: By = By::Id("login_info_win_close");
    const STATION_LINK_SELECTOR: By = By::XPath(r"// tr / td[3] // a");
    const NEXT_PAGE_SELECTOR: By = By::ClassName("ant-pagination-next");

    const STATION_LINK_REGEX: &str = include_str!("stationlink.secret.txt");

    let href_match = Regex::new(STATION_LINK_REGEX).unwrap();
    try {
        let mut stations = Vec::with_capacity(128);

        tracing::trace!("waiting for site to load");
        driver
            .query(CLOSE_TOAST_SELECTOR)
            .and_clickable()
            .single()
            .await?
            .click()
            .await?;
        tracing::trace!("closed login toast");

        loop {
            tracing::trace!("searching for stations in page");

            let links = driver.query(STATION_LINK_SELECTOR).all_required().await?;

            tracing::trace!("processing stations");
            for link in &links {
                let name = link.text().await?;

                let href = link
                    .get_attribute("href")
                    .await
                    .with_context(|| "missing link in station name")?
                    .with_context(|| "link is not a string")?;

                let id = href_match
                    .captures(&href)
                    .and_then(|cap| cap.name("id"))
                    .map(|cap| String::from(cap.as_str()))
                    .with_context(|| "missing station id in link")?;

                tracing::trace!(%id, %name, "found station");
                stations.push(Station { id, name });
            }
            tracing::debug!(
                count = links.len(),
                total = stations.len(),
                "stations processed"
            );

            let next = driver.query(NEXT_PAGE_SELECTOR).single().await?;

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
async fn export_report(driver: &mut WebDriver, station: &Station) -> Result<Vec<Record>> {
    const REPORT_PAGE_TEMPLATE: &str = include_str!("reportpage.secret.txt");

    // language=Xpath
    const TIME_DROPDOWN_SELECTOR: By =
        By::XPath(r#"// *[@class="nco-site-search-bar"] // *[@class="ant-select-selection-item"]"#);
    // language=Xpath
    const MONTH_OPTION_SELECTOR: By =
        By::XPath(r#"// *[@class="nco-site-search-bar"] // *[@title="Monthly"] / *[1]"#);

    // language=Xpath
    const DATE_SELECTOR: By = By::XPath(
        r#"// tbody[@class="ant-table-tbody"] / tr[contains(@class, 'ant-table-row')] / td[1]"#,
    );
    // language=Xpath
    const YIELD_SELECTOR: By = By::XPath(
        r#"// tbody[@class="ant-table-tbody"] / tr[contains(@class, 'ant-table-row')] / td[2]"#,
    );

    // language=Xpath
    const PAGE_DROPDOWN_SELECTOR: By = By::XPath(
        r#"// *[@class="ant-pagination-options"] // *[@class="ant-select-selection-item"]"#,
    );
    // language=Xpath
    const ALL_OPTION_SELECTOR: By =
        By::XPath(r#"// *[@class="ant-pagination-options"] // *[@title="50 / page"] / *[1]"#);

    try {
        tracing::debug!("accessing reports");
        driver
            .get(format!("{}{}", REPORT_PAGE_TEMPLATE, station.id))
            .await?;
        driver.refresh().await?;

        tracing::debug!("setting page size");
        driver
            .query(PAGE_DROPDOWN_SELECTOR)
            .single()
            .await?
            .click()
            .await?;

        tracing::trace!("picking 50 records per page");
        let size_opt = driver.query(ALL_OPTION_SELECTOR).single().await?;
        size_opt.wait_until().clickable().await?;
        size_opt.click().await?;

        tracing::debug!("setting time granularity");
        driver
            .query(TIME_DROPDOWN_SELECTOR)
            .single()
            .await?
            .click()
            .await?;

        tracing::trace!("picking monthly granularity");
        let table_opt = driver.query(MONTH_OPTION_SELECTOR).single().await?;
        table_opt.wait_until().clickable().await?;
        table_opt.click().await?;

        wait_for_table_reload(driver).await?;

        tracing::trace!("scanning for dates");
        let dates = driver.query(DATE_SELECTOR).all().await?;

        tracing::trace!("scanning for yields");
        let yields = driver.query(YIELD_SELECTOR).all().await?;

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

            let date = date.text().await?;
            let pv_yield = pv_yield.text().await?.parse().ok();

            tracing::trace!(%date, "yield" = ?pv_yield, "added record");
            data.push(Record { date, pv_yield })
        }

        tracing::debug!("station scraped");
        data
    }
}

async fn wait_for_table_reload(driver: &mut WebDriver) -> Result<()> {
    const WAITER_SELECTOR: By = By::ClassName("ant-spin-container");

    try {
        tracing::trace!("waiting for table reload");
        driver
            .query(WAITER_SELECTOR)
            .single()
            .await?
            .wait_until()
            .lacks_class(StringMatch::new("ant-spin-blur").partial())
            .await?
    }
}
