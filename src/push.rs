use crate::config;

use std::time;

use anyhow::Result;
use log::info;

#[derive(Debug, Default, Clone)]
pub struct PushMessage {
    pub is_warning: bool,
    pub title: String,
    pub msg: String,
}

pub fn push(config: &config::Config, msg: PushMessage) -> Result<()> {
    info!("Report: {:?}", msg);
    if msg.is_warning || config.push_daily_report {
        let mut pushed = false;
        if !config.push_ftqq_key.is_empty() {
            push_ftqq(config, msg.clone())?;
            pushed = true;
        }
        if !config.is_smtp_empty() {
            push_smtp(config, msg.clone())?;
            pushed = true
        }
        if !pushed {
            info!("Do not set push method");
        }
    }
    Ok(())
}

fn push_ftqq(config: &config::Config, msg: PushMessage) -> Result<()> {
    let _ = ureq::get(format!(
        "https://sctapi.ftqq.com/{}.send",
        config.push_ftqq_key
    ))
    .query("title", msg.title)
    .query("desp", msg.msg)
    .call()?;
    Ok(())
}

fn push_smtp(config: &config::Config, msg: PushMessage) -> Result<()> {
    use lettre::{
        Message, SmtpTransport, Transport,
        transport::smtp::{authentication::Credentials, client::Tls},
    };
    let email = Message::builder()
        .from(format!("charge-monitor <{}>", config.email_smtp_user).parse()?)
        .to(config.email_recipient.parse()?)
        .subject(msg.title)
        .body(msg.msg)?;
    let creds = Credentials::new(
        config.email_smtp_user.to_owned(),
        config.email_smtp_password.to_owned(),
    );
    let mailer = SmtpTransport::builder_dangerous(&config.email_smtp_server)
        .port(465)
        .tls(Tls::Wrapper(
            lettre::transport::smtp::client::TlsParameters::new(config.email_smtp_server.clone())?,
        ))
        .timeout(Some(time::Duration::new(2, 0)))
        .credentials(creds)
        .build();
    mailer.send(&email)?;
    Ok(())
}
