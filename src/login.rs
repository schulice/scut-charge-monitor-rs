use std::sync::Arc;
use std::{collections::HashMap, sync::LazyLock};

use anyhow::{Context, Result};
use reqwest::{
    Client, Url,
    cookie::{CookieStore, Jar},
    header::{self, HeaderMap, HeaderValue},
    redirect::Policy,
};
use serde::Deserialize;
use serde_json::json;

use crate::config;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct CaptchaResp {
    key: String,
    image: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardInfo {
    uuid: String,
    number_keyboard: String,
    lower_letter_keyboard: String,
    upper_letter_keyboard: String,
}

static STANDERAD_KEYBOARD_INFO: LazyLock<KeyboardInfo> = LazyLock::new(|| KeyboardInfo {
    uuid: 0.to_string(),
    number_keyboard: "0123456789".to_string(),
    lower_letter_keyboard: "abcdefghijklmnopqrstuvwxyz".to_string(),
    upper_letter_keyboard: "ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_string(),
});

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct FetchKeyboardInfoResp {
    data: KeyboardInfo,
    code: i32,
    success: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum LoginError {
    #[error("Can not pass authorication")]
    AuthorizationFailed,
    #[error("Invalid password or username")]
    InvalidPassword,
    #[error("Invalid captcha code")]
    InvalidCaptchaCode,
    #[error("Http connection error")]
    NotOkHttp(reqwest::StatusCode),
    #[error("Logical request error")]
    NotOkErrorCode(i32),
    #[error("Network error")]
    NetworkError(#[from] reqwest::Error),
    #[error("Innner error")]
    InnerError(#[from] anyhow::Error),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LoginResp {
    error_code: i32,
    access_token: String,
}

pub struct LoginSession {
    cookie_jar: Arc<Jar>,
}

impl LoginSession {
    pub fn new() -> Self {
        Self {
            cookie_jar: Arc::new(Jar::default()),
        }
    }

    pub fn from_jar(jar: Arc<Jar>) -> Self {
        Self { cookie_jar: jar }
    }
}

impl LoginSession {
    fn default_client(&self) -> Result<Client> {
        let mut header = HeaderMap::new();
        header.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static(config::AUTHORIZATION),
        );
        let client = Client::builder()
            .cookie_provider(self.cookie_jar.clone())
            .user_agent(config::USER_AGENT)
            .default_headers(header)
            .build()?;
        Ok(client)
    }

    pub async fn fetch_keyboard_info(&self, captcha_key: &str) -> Result<KeyboardInfo> {
        let client = self.default_client()?;
        let url = config::url_keyboard_from_captcha_key(captcha_key);
        let resp = client.get(url).send().await?;
        let resp = resp
            .error_for_status()
            .context("failed to get keyboard info")?;
        let data: FetchKeyboardInfoResp =
            resp.json().await.context("parser keyboard info error")?;
        let info = data.data;
        Ok(info)
    }

    pub async fn get_captcha_data(&self) -> Result<(String, String)> {
        let resp = self
            .default_client()?
            .get(config::URL_CAPTCHA)
            .send()
            .await
            .context("Failed to send captcha request")?;
        let resp = resp
            .error_for_status()
            .context("Captcha request return an error status")?;
        let data: CaptchaResp = resp.json().await.context("parser captcha json error")?;
        let key = data.key;
        let img_base64 = data
            .image
            .split(',')
            .last()
            .ok_or_else(|| anyhow::anyhow!("split img from captcha data failed"))?
            .to_string();
        Ok((key, img_base64))
    }

    fn encrpy_with_dynamic_keyboard(passwd: &str, info: &KeyboardInfo) -> Result<String> {
        let mut mapping: HashMap<char, char> = HashMap::new();
        mapping.extend(
            STANDERAD_KEYBOARD_INFO
                .number_keyboard
                .chars()
                .zip(info.number_keyboard.chars()),
        );
        mapping.extend(
            STANDERAD_KEYBOARD_INFO
                .lower_letter_keyboard
                .chars()
                .zip(info.lower_letter_keyboard.chars()),
        );
        mapping.extend(
            STANDERAD_KEYBOARD_INFO
                .upper_letter_keyboard
                .chars()
                .zip(info.upper_letter_keyboard.chars()),
        );
        let passwd: String = passwd.chars().map(|c| mapping.get(&c).unwrap()).collect();
        let passwd = format!("{}$1${}", passwd, info.uuid);
        Ok(passwd)
    }

    /// `img` is base64
    pub async fn recognize_captcha(
        &self,
        config: &config::Config,
        img: String,
    ) -> Result<Vec<String>> {
        let prompt_text = "Analyze this CAPTCHA image. \
        The CAPTCHA consists of English letters and numbers. \
        Please return a JSON array containing the 3 most likely results, \
        sorted from highest to lowest probability. \
        For example: [\"abcd\", \"abce\", \"abcf\"]. \
        Please strictly adhere to the JSON format and do not include any additional explanatory text.\
        And DO NOT wrap them in markdown block.";

        let request_body = json!({
            "model": config.llm_model,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": prompt_text
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", img)
                        }
                    }
                ]
            }],
            "max_tokens": 300,
            "stream": false
        });

        let completions_url = format!("{}/chat/completions", config.llm_api_base);
        let response = Client::new()
            .post(&completions_url)
            .bearer_auth(config.llm_api_key.clone())
            .json(&request_body)
            .send()
            .await
            .context("failed to send request when recorgnize captcha")?;
        let response_text = response.text().await?;
        let value: serde_json::Value =
            serde_json::from_str(&response_text).context("parser llm recorgnize captcha error")?;
        let path = "/choices/0/message/content";
        let result = value
            .pointer(path)
            .and_then(|value| value.as_str())
            .and_then(|s| serde_json::from_str(s).ok())
            .ok_or_else(|| anyhow::anyhow!("parser from llm response error"))?;
        Ok(result)
    }

    fn get_jsessionid_from(jar: &Jar, domain: &str) -> Result<String> {
        let url = Url::parse(domain)?;
        let value: HeaderValue = jar
            .cookies(&url)
            .ok_or_else(|| anyhow::anyhow!("do not find domain in jar"))?;
        let value: HashMap<_, _> = value
            .to_str()?
            .split(';')
            .filter_map(|x| {
                let mut kv = x.trim().splitn(2, '=');
                match (kv.next(), kv.next()) {
                    (Some(k), Some(v)) if !k.is_empty() => Some((k.to_string(), v.to_string())),
                    _ => None,
                }
            })
            .collect();
        value
            .get("JSESSIONID")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("do not find value in jar"))
    }

    pub async fn perform_redirect(&self, access_token: &str) -> Result<String> {
        let req = json!(
            {
                "appId": "360",
                "loginFrom": "h5",
                "synAccessSource": "h5",
                "synjones-auth": access_token,
                "type": "app"
            }
        );
        let cli = Client::builder()
            .cookie_provider(self.cookie_jar.clone())
            .redirect(Policy::none())
            .build()?;
        let resp = cli.get(config::URL_REDIRECT).json(&req).send().await?;
        let resp = resp.error_for_status()?;
        let jar = self.cookie_jar.clone();
        match Self::get_jsessionid_from(&jar, config::DOMAIN_DFYC) {
            Ok(id) => Ok(id),
            Err(_) => {
                let next_url = resp
                    .headers()
                    .get("Location")
                    .ok_or_else(|| anyhow::anyhow!("do not find localtion section on header"))?
                    .to_str()?;
                let cli = Client::builder()
                    .cookie_provider(self.cookie_jar.clone())
                    .build()?;
                cli.get(next_url).send().await?;
                Self::get_jsessionid_from(&jar, config::DOMAIN_DFYC)
                    .context("failed to redirect to find dfyc domain ")
            }
        }
    }

    pub async fn login(
        &self,
        username: &str,
        encryped_passwd: &str,
        captcha_key: &str,
        captcha_code: &str,
    ) -> Result<String, LoginError> {
        let req = json!(
            {
                "grant_type": "password",
                "scope": "all",
                "username": username,
                "password": encryped_passwd,
                "logintype": "card",
                "captcha_header_code": captcha_code,
                "captcha_header_key": captcha_key,
                "loginFrom": "h5",
                "device_token": "h5",
                "synAccessSource": "h5"
            }
        );
        let resp = self
            .default_client()?
            .get(config::URL_LOGIN)
            .json(&req)
            .send()
            .await?;
        use reqwest::StatusCode;
        let status = resp.status();
        match status {
            StatusCode::OK => {
                let resp: LoginResp = resp.json().await?;
                Ok(resp.access_token)
            }
            StatusCode::BAD_REQUEST => {
                let resp: LoginResp = resp.json().await?;
                match resp.error_code {
                    8000 => Err(LoginError::InvalidPassword),
                    8002 => Err(LoginError::InvalidCaptchaCode),
                    _ => Err(LoginError::NotOkErrorCode(resp.error_code)),
                }
            }
            StatusCode::UNAUTHORIZED => Err(LoginError::AuthorizationFailed),
            _ => Err(LoginError::NotOkHttp(status)),
        }
    }

    pub async fn process(&self, config: &config::Config) -> Result<()> {
        let (captcha_key, captchar_img_base64) = self.get_captcha_data().await?;
        let info = self.fetch_keyboard_info(&captcha_key).await?;
        let captcha_candidates = self.recognize_captcha(config, captchar_img_base64).await?;
        let passwd_encrypted = Self::encrpy_with_dynamic_keyboard(&config.scut_password, &info)?;
        let mut errors = Vec::new();
        for captcha_code in captcha_candidates.iter() {
            let login_result = self
                .login(
                    &config.scut_username,
                    &passwd_encrypted,
                    &captcha_key,
                    &captcha_code,
                )
                .await;
            match login_result {
                Ok(token) => {
                    self.perform_redirect(&token).await?;
                    return Ok(());
                }
                Err(e) => errors.push(e),
            }
        }
        let error_report = errors
            .into_iter()
            .enumerate()
            .map(|(i, e)| format!("  Attempt {}: {:?}", i + 1, e))
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::bail!(
            "Operation failed after {} attempts with the following errors:\n{}",
            captcha_candidates.len(),
            error_report
        );
    }
}
