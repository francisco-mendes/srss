use std::ops::ControlFlow;

use anyhow::{
    Context,
    Result,
};
use thirtyfour::{
    components::{
        Component,
        ElementResolver,
    },
    error::WebDriverResult,
    prelude::{
        ElementQueryable,
        WebDriverError,
    },
    By,
    Key,
    WebDriver,
    WebElement,
};

use crate::{
    cli::Credentials,
    scraper::retry_loop,
};

#[derive(Debug, Clone, Component)]
pub struct Login {
    #[base]
    form: WebElement,
    #[by(id = "username", description = "username input")]
    username: ElementResolver<WebElement>,
    #[by(id = "value", description = "password input")]
    password: ElementResolver<WebElement>,
    #[by(id = "submitDataverify", description = "login button")]
    login: ElementResolver<WebElement>,
}

impl Login {
    pub async fn query(driver: &WebDriver) -> Result<Self> {
        try {
            let element = driver
                .query(By::Id("loginFormArea"))
                .and_displayed()
                .and_enabled()
                .single()
                .await
                .context("failed to find login form")?;
            Self::new(element)
        }
    }

    pub async fn login(&self, driver: &WebDriver, credentials: Credentials) -> Result<()> {
        try {
            let user_input = self
                .username
                .resolve_present()
                .await
                .context("unable to find username input")?;
            let pass_input = self
                .password
                .resolve_present()
                .await
                .context("unable to find password input")?;
            let login_button = self
                .login
                .resolve_present()
                .await
                .context("unable to find login button")?;
            driver
                .action_chain()
                .send_keys_to_element(&user_input, &credentials.username)
                .send_keys_to_element(&pass_input, &credentials.password)
                .click_element(&login_button)
                .perform()
                .await
                .context("unable to execute login")?;
        }
    }
}

#[derive(Debug, Clone, Component)]
pub struct ControlForm {
    #[base]
    form: WebElement,
    #[by(
        id = "site-report-nco-tree-select-customized",
        description = "plant selector"
    )]
    plants: ElementResolver<StationTree>,
    #[by(id = "timeDimension", description = "time granularity selector")]
    granularity: ElementResolver<WebElement>,
    #[by(id = "statisticTime", description = "time period selector")]
    time_period: ElementResolver<WebElement>,
    #[by(css = "button.ant-btn.ant-btn-primary", description = "search button")]
    search: ElementResolver<WebElement>,
}

impl ControlForm {
    pub async fn query(driver: &WebDriver) -> Result<Self> {
        retry_loop("finding control bar", || async {
            // language=Css
            let selector: By = By::Css("form.ant-form.ant-form-inline");
            let element = driver
                .query(selector)
                .and_displayed()
                .and_enabled()
                .single()
                .await?;
            Ok(ControlFlow::Break(Self::new(element)))
        })
        .await
        .context("failed to find control bar")
    }

    pub async fn set_data_period(&self, driver: &WebDriver, month: Option<String>) -> Result<()> {
        retry_loop("setting data time granularity", || async {
            tracing::trace!("querying granularity selector");
            let granularity = self.granularity.resolve().await?;

            tracing::trace!("setting monthly granularity");
            driver
                .action_chain()
                .send_keys_to_element(&granularity, Key::Down + Key::Enter)
                .perform()
                .await?;

            tracing::trace!("checking if granularity is set successfully");
            // language=Css
            let selector = By::Css("span[title=Monthly]");
            let _ = self.form.query(selector).and_displayed().single().await?;

            tracing::trace!("set data time granularity");
            Ok(ControlFlow::BREAK)
        })
        .await
        .context("setting time granularity to monthly")?;

        if let Some(month) = month.as_deref() {
            retry_loop("setting time period", || async {
                let time_period = self.time_period.resolve_present().await?;
                driver
                    .action_chain()
                    .click_element(&time_period)
                    .key_down(Key::Control)
                    .send_keys("a")
                    .key_up(Key::Control)
                    .send_keys(Key::Backspace + month + Key::Enter)
                    .perform()
                    .await?;

                if self
                    .time_period
                    .resolve_present()
                    .await?
                    .value()
                    .await?
                    .unwrap_or_default()
                    == month
                {
                    tracing::trace!("set data time period");
                    Ok(ControlFlow::BREAK)
                } else {
                    Ok(ControlFlow::CONTINUE)
                }
            })
            .await
            .context("failed to set data time period")?
        }
        Ok(())
    }

    pub async fn stations(&self) -> Result<Vec<StationLine>> {
        try {
            self.plants
                .resolve()
                .await
                .context("failed to find station tree")?
                .stations()
                .await?
        }
    }

    pub async fn click_stations(&self) -> Result<()> {
        try {
            self.plants
                .resolve_present()
                .await
                .context("failed to find station input")?
                .base
                .click()
                .await
                .context("failed to click on station input")?
        }
    }

    pub async fn click_search(&self) -> Result<()> {
        try {
            self.search
                .resolve_present()
                .await
                .context("failed to find search button")?
                .click()
                .await
                .context("failed to click on search button")?
        }
    }
}

#[derive(Debug, Clone, Component)]
pub struct Pagination {
    #[base]
    form: WebElement,
    #[by(
        css = ".ant-select-selection-search > .ant-select-selection-search-input",
        description = "page size"
    )]
    page_size: ElementResolver<WebElement>,
}

impl Pagination {
    pub async fn query(driver: &WebDriver) -> Result<Self> {
        retry_loop("finding pagination controls", || async {
            // language=Css
            let selector: By = By::Css(".ant-pagination-options");
            let element = driver
                .query(selector)
                .and_displayed()
                .and_enabled()
                .single()
                .await?;
            Ok(ControlFlow::Break(Self::new(element)))
        })
        .await
        .context("failed to find pagination controls")
    }

    pub async fn increase_page_size(&self, driver: &WebDriver) -> Result<()> {
        retry_loop("increasing page size", || async {
            let page_size = self.page_size.resolve_present().await?;
            driver
                .action_chain()
                .send_keys_to_element(&page_size, Key::Down + Key::Down + Key::Down + Key::Enter)
                .perform()
                .await?;

            if self
                .form
                .query(By::Css(".ant-select-selection-item"))
                .and_displayed()
                .single()
                .await?
                .attr("title")
                .await?
                .unwrap_or_default()
                == "50 / page"
            {
                tracing::trace!("set table page size");
                Ok(ControlFlow::BREAK)
            } else {
                Ok(ControlFlow::CONTINUE)
            }
        })
        .await
        .context("setting page size")
    }
}

#[derive(Debug, Clone, Component)]
pub struct StationTree {
    #[base]
    base: WebElement,
    #[by(css = "li > .flex-node-line", description = "stations")]
    stations: ElementResolver<Vec<StationLine>>,
}

impl StationTree {
    pub async fn stations(&self) -> Result<Vec<StationLine>> {
        self.stations
            .resolve_present()
            .await
            .context("failed to find stations")
    }
}

#[derive(Debug, Clone, Component)]
pub struct StationLine {
    #[base]
    line: WebElement,
    #[by(nowait, css = ".anticon.tree-icon", description = "station directory")]
    caret: ElementResolver<WebElement>,
    #[by(css = "input.node-line-checkbox", description = "station checkbox")]
    checkbox: ElementResolver<WebElement>,
    #[by(css = ".flex-node-line-name-part", description = "station name")]
    station_name: ElementResolver<WebElement>,
}

impl StationLine {
    pub async fn station_name(&self) -> String {
        let result: Result<String> = try {
            self.station_name
                .resolve_present()
                .await
                .context("failed to find line name")?
                .attr("title")
                .await
                .context("failed to find station name")?
                .context("missing station name")?
        };
        match result {
            Ok(name) => name,
            Err(err) => {
                tracing::warn!("unable to find station name: {err}");
                String::from("<missing>")
            }
        }
    }

    pub async fn is_dir(&self) -> Result<bool> {
        match self.caret.resolve_present().await {
            Err(WebDriverError::NoSuchElement(_)) => Ok(false),
            Ok(_) => Ok(true),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn click(&self) -> Result<()> {
        let name = self.station_name().await;

        match self.caret.resolve_present().await {
            Ok(caret) => caret
                .click()
                .await
                .with_context(|| format!("unable to open station directory '{name}'")),
            Err(WebDriverError::NoSuchElement(_)) => self
                .checkbox
                .resolve_present()
                .await
                .with_context(|| format!("unable to find checkbox for station '{name}'"))?
                .click()
                .await
                .with_context(|| format!("unable to click station '{name}'")),
            Err(err) => Err(err.into()),
        }
    }
}
