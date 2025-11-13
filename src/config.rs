#![allow(dead_code)]

use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(rename_all(deserialize = "snake_case"))]
pub struct Config {
    #[serde(alias = "SCUT_USERNAME")]
    pub scut_username: String,
    #[serde(alias = "SCUT_PASSWORD")]
    pub scut_password: String,
    #[serde(alias = "LLM_MODEL")]
    pub llm_model: String,
    #[serde(alias = "LLM_API_KEY")]
    pub llm_api_key: String,
    #[serde(alias = "LLM_API_BASE")]
    pub llm_api_base: String,
    #[serde(alias = "ELECTRICITY_ALERT_THRESHOLD")]
    pub electricity_alert_threshold: i32,
    #[serde(alias = "LLM_RECOGNITION_RETRIES")]
    pub llm_recognition_retries: i32,
    #[serde(alias = "PUSH_FTQQ_KEY")]
    pub push_ftqq_key: String,
    #[serde(alias = "PUSH_DAILY_REPORT")]
    pub push_daily_report: bool,
    #[serde(skip, alias = "EMAIL_SMTP_SERVER")]
    pub email_smtp_server: String,
    #[serde(skip, alias = "EMAIL_SMTP_PORT")]
    pub email_smtp_port: i32,
    #[serde(skip, alias = "EMAIL_SMTP_USER")]
    pub email_smtp_user: String,
    #[serde(skip, alias = "EMAIL_SMTP_PASSWORD")]
    pub email_smtp_password: String,
    #[serde(skip, alias = "EMAIL_RECIPIENT")]
    pub email_recipient: String,
}

impl Config {
    pub fn read_from_env() -> Result<Self> {
        serde_envfile::from_env().context("failed to read env parser")
    }
    pub fn read_from_jsonfile(file: &Path) -> Result<Self> {
        let content = fs::read_to_string(file)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }
    pub fn read_from_envfile(file: &Path) -> Result<Self> {
        let content = fs::read_to_string(file)?;
        let config: Config = serde_envfile::from_str(&content)?;
        Ok(config)
    }

    pub fn read_from_file(file: &Path) -> Result<Self> {
        let name = file
            .file_name()
            .context("get path config file name error")?;
        match name.to_str() {
            Some(".env") => Self::read_from_envfile(file),
            Some(_) => {
                let extension = file
                    .extension()
                    .context("get config path extension failed")?;
                match extension.to_str() {
                    Some("json") => Self::read_from_envfile(file),
                    Some("env") => Self::read_from_envfile(file),
                    _ => Err(anyhow::anyhow!("can not parser config file extension")),
                }
            }
            _ => Err(anyhow::anyhow!("file name cannot parser to str")),
        }
    }
}

pub const DOMAIN_DFYC: &str = "dfyc.utc.scut.edu.cn";
pub const DOMAIN_LOGIN: &str = "ecardwxnew.scut.edu.cn";

pub const URL_CHARGE_HISTORY: &str =
    "https://dfyc.utc.scut.edu.cn/sdms-weixin-pay-sp/service/ele/list";

pub const URL_BASE: &str = "https://ecardwxnew.scut.edu.cn";
pub const URL_REDIRECT: &str = "https://ecardwxnew.scut.edu.cn/berserker-base/redirect";
pub const URL_CAPTCHA: &str = "https://ecardwxnew.scut.edu.cn/berserker-auth/oauth/captcha";
pub const URL_LOGIN: &str = "https://ecardwxnew.scut.edu.cn/berserker-auth/oauth/token";

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36";
pub const BASIC_AUTHORIZATION: &str =
    "bW9iaWxlX3NlcnZpY2VfcGxhdGZvcm06bW9iaWxlX3NlcnZpY2VfcGxhdGZvcm1fc2VjcmV0";

pub fn url_keyboard_from_captcha_key(key: &str) -> String {
    format!(
        "{URL_BASE}/berserker-secure/keyboard?type=Standard&order=0&synAccessSource=h5&key={}",
        key
    )
}
