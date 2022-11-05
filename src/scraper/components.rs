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
    prelude::ElementQueryable,
    By,
    Key,
    WebDriver,
    WebElement,
};

use crate::cli::Credentials;

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
                .resolve()
                .await
                .context("unable to find username input")?;
            let pass_input = self
                .password
                .resolve()
                .await
                .context("unable to find password input")?;
            let login_button = self
                .login
                .resolve()
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
    plants: ElementResolver<WebElement>,
    #[by(id = "timeDimension", description = "time granularity selector")]
    granularity: ElementResolver<WebElement>,
    #[by(id = "statisticTime", description = "time period selector")]
    time_period: ElementResolver<WebElement>,
    #[by(css = "button.ant-btn.ant-btn-primary", description = "search button")]
    search: ElementResolver<WebElement>,
}

impl ControlForm {
    pub async fn query(driver: &WebDriver) -> Result<Self> {
        try {
            // language=Css
            let selector: By = By::Css("form.ant-form.ant-form-inline");
            let element = driver
                .query(selector)
                .and_displayed()
                .and_enabled()
                .single()
                .await
                .context("failed to find control bar")?;
            Self::new(element)
        }
    }

    pub async fn set_data_period(&self, driver: &WebDriver, month: Option<String>) -> Result<()> {
        try {
            loop {
                let granularity = self
                    .granularity
                    .resolve()
                    .await
                    .context("failed to find time granularity input")?;

                driver
                    .action_chain()
                    .send_keys_to_element(&granularity, Key::Down + Key::Enter)
                    .perform()
                    .await
                    .context("unable to select monthly granularity")?;

                if granularity
                    .attr("aria-activedescendant")
                    .await
                    .context("missing granularity value")?
                    .unwrap_or_default()
                    == "timeDimension_list_1"
                {
                    tracing::trace!("set data time granularity");
                    break;
                } else {
                    tracing::warn!("unable to set data time granularity, retrying...");
                }
            }

            if let Some(month) = month.as_deref() {
                loop {
                    let time_period = self
                        .time_period
                        .resolve()
                        .await
                        .context("unable to find time input")?;
                    driver
                        .action_chain()
                        .click_element(&time_period)
                        .key_down(Key::Control)
                        .send_keys("a")
                        .key_up(Key::Control)
                        .send_keys(Key::Backspace + month + Key::Enter)
                        .perform()
                        .await
                        .context("unable to set month")?;

                    if time_period
                        .value()
                        .await
                        .context("missing time period value")?
                        .unwrap_or_default()
                        == month
                    {
                        tracing::trace!("set data time period");
                        break;
                    } else {
                        tracing::warn!("unable to set data time period, retrying...");
                    }
                }
            }
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
        try {
            // language=Css
            let selector: By = By::Css(".ant-pagination-options");
            let element = driver
                .query(selector)
                .and_displayed()
                .and_enabled()
                .single()
                .await
                .context("failed to find pagination form")?;
            Self::new(element)
        }
    }

    pub async fn increase_page_size(&self, driver: &WebDriver) -> Result<()> {
        try {
            loop {
                let page_size = self
                    .page_size
                    .resolve()
                    .await
                    .context("failed to find pagination input")?;

                driver
                    .action_chain()
                    .send_keys_to_element(
                        &page_size,
                        Key::Down + Key::Down + Key::Down + Key::Enter,
                    )
                    .perform()
                    .await
                    .context("failed to set table page size to 50")?;

                if page_size
                    .attr("aria-activedescendant")
                    .await
                    .context("missing granularity value")?
                    .unwrap_or_default()
                    == "rc_select_3_list_3"
                {
                    tracing::trace!("set table page size");
                    break;
                } else {
                    tracing::warn!("unable to set table page size, retrying...");
                }
            }
        }
    }
}
