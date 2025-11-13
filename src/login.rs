use std::time::Duration;
use std::{collections::HashMap, sync::LazyLock};

use anyhow::{Context, Result};
use log::info;
use serde::Deserialize;
use serde_json::json;
use ureq::{Agent, http::StatusCode};

use crate::config::{self};

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
    #[error("Can not pass authorization")]
    AuthorizationFailed { message: String },
    #[error("Invalid password or username")]
    InvalidPassword,
    #[error("Invalid captcha code")]
    InvalidCaptchaCode,
    #[error("Http connection error")]
    NotOkHttp(StatusCode),
    #[error("Logical request error")]
    NotOkErrorCode(i32),
    #[error("Network error")]
    NetworkError(#[from] ureq::Error),
    #[error("Innner error")]
    InnerError(#[from] anyhow::Error),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LoginResp {
    #[serde(default)]
    code: i32,
    status: i32,
    message: String,
    #[serde(default)]
    access_token: String,
}

pub struct LoginSession {
    pub agent: ureq::Agent,
}

impl LoginSession {
    pub fn new() -> Result<Self> {
        let config = Agent::config_builder()
            .user_agent(config::USER_AGENT)
            .ip_family(ureq::config::IpFamily::Ipv4Only)
            .redirect_auth_headers(ureq::config::RedirectAuthHeaders::SameHost)
            .http_status_as_error(false)
            .build();
        Ok(Self {
            agent: config.into(),
        })
    }
}

impl LoginSession {
    pub fn fetch_keyboard_info(&self, captcha_key: &str) -> Result<KeyboardInfo> {
        let agent = &self.agent;
        let url = config::url_keyboard_from_captcha_key(captcha_key);
        let mut resp = agent.get(url).call()?;
        if resp.status() != StatusCode::OK {
            anyhow::bail!("failed to get keyboard info, status: {}", resp.status());
        }
        let data: FetchKeyboardInfoResp = resp
            .body_mut()
            .read_json()
            .context("parser keyboard info error")?;
        let info = data.data;
        Ok(info)
    }

    pub fn get_captcha_data(&self) -> Result<(String, String)> {
        let mut resp = self
            .agent
            .get(config::URL_CAPTCHA)
            .header(
                "authorization",
                format!("Basic {}", config::BASIC_AUTHORIZATION),
            )
            .call()?;
        if resp.status() != StatusCode::OK {
            anyhow::bail!("Captcha request return an error status: {}", resp.status());
        }
        let data: CaptchaResp = resp.body_mut().read_json()?;
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
    pub fn recognize_captcha(&self, config: &config::Config, img: String) -> Result<Vec<String>> {
        info!("recogize_captcha");
        let prompt = r#""
            Analyze this CAPTCHA image.
            The CAPTCHA consists of English letters and numbers.
            Please return a JSON array containing the 3 most likely results.
            Each result is consisted of 5 char or number, and no blank.
            For example: ["abc2g", "abce1", "ab0el"]. 
            Please strictly adhere to the JSON format and do not include any additional explanatory text.
            And DO NOT wrap them in markdown block.
        ""#;
        let request_body = json!({
            "model": config.llm_model,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": prompt
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
        let mut resp = ureq::post(&completions_url)
            .header("authorization", format!("Bearer {}", config.llm_api_key))
            .send_json(&request_body)?;
        let resp = resp.body_mut().read_to_string()?;
        let value: serde_json::Value =
            serde_json::from_str(&resp).context("parser llm recorgnize captcha error")?;
        let path = "/choices/0/message/content";
        let result = value
            .pointer(path)
            .and_then(|value| value.as_str())
            .and_then(|s| serde_json::from_str(s).ok())
            .ok_or_else(|| anyhow::anyhow!("parser from llm response error"))?;
        Ok(result)
    }

    pub fn perform_redirect(&self, access_token: &str) -> Result<String> {
        let req = json!(
            {
                "appId": "360",
                "loginFrom": "h5",
                "synAccessSource": "h5",
                "synjones-auth": access_token,
                "type": "app"
            }
        );
        // TODO not redirect here
        let resp = self
            .agent
            .get(config::URL_REDIRECT)
            .force_send_body()
            .send_json(&req)?;
        unimplemented!("check redirect sucess")
    }

    pub fn login(
        &self,
        username: &str,
        encrypted_password: &str,
        captcha_key: &str,
        captcha_code: &str,
    ) -> Result<String, LoginError> {
        let req = json!(
            {
                "grant_type": "password",
                "scope": "all",
                "username": username,
                "password": encrypted_password,
                "logintype": "card",
                "captcha_header_code": captcha_code,
                "captcha_header_key": captcha_key,
                "loginFrom": "h5",
                "device_token": "h5",
                "synAccessSource": "h5",
            }
        );
        let mut resp = self
            .agent
            .post(config::URL_LOGIN)
            .header(
                "authorization",
                format!("Basic {}", config::BASIC_AUTHORIZATION),
            )
            .send_json(&req)?;
        let status = resp.status();
        match status {
            StatusCode::OK => {
                let resp: LoginResp = resp.body_mut().read_json()?;
                Ok(resp.access_token)
            }
            StatusCode::BAD_REQUEST => {
                let resp: LoginResp = resp.body_mut().read_json()?;
                info!("resp: {:?}", resp);
                match resp.code {
                    8000 => Err(LoginError::InvalidPassword),
                    8002 => Err(LoginError::InvalidCaptchaCode),
                    _ => Err(LoginError::NotOkErrorCode(resp.code)),
                }
            }
            StatusCode::UNAUTHORIZED => {
                let resp: LoginResp = resp.body_mut().read_json()?;
                Err(LoginError::AuthorizationFailed {
                    message: resp.message,
                })
            }
            _ => Err(LoginError::NotOkHttp(status)),
        }
    }

    pub fn process(&self, config: &config::Config) -> Result<()> {
        let mut errors = Vec::new();
        for _ in 0..config.llm_recognition_retries {
            let (captcha_key, captchar_img_base64) = self.get_captcha_data()?;
            let info = self.fetch_keyboard_info(&captcha_key)?;
            let captcha_candidates = self.recognize_captcha(config, captchar_img_base64)?;
            let passwd_encrypted =
                Self::encrpy_with_dynamic_keyboard(&config.scut_password, &info)?;
            for captcha_code in captcha_candidates.iter() {
                let login_result = self.login(
                    &config.scut_username,
                    &passwd_encrypted,
                    &captcha_key,
                    &captcha_code,
                );
                match login_result {
                    Ok(token) => {
                        self.perform_redirect(&token)?;
                        return Ok(());
                    }
                    Err(e) => errors.push(e),
                }
            }
            std::thread::sleep(Duration::from_millis(1000));
        }
        let error_count = errors.len();
        let error_report = errors
            .into_iter()
            .enumerate()
            .map(|(i, e)| format!("  Attempt {}: {:?}", i + 1, e))
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::bail!(
            "Operation failed after {} attempts with the following errors:\n{}",
            error_count,
            error_report
        );
    }
}
