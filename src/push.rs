use crate::config;

use anyhow::Result;
use log::info;

#[derive(Debug, Default)]
pub struct PushMessage {
    pub is_warning: bool,
    pub title: String,
    pub msg: String,
}

// now we just support ftqq server jiang
pub fn push(config: &config::Config, msg: PushMessage) -> Result<()> {
    info!("Report: {:?}", msg);
    if msg.is_warning || config.push_daily_report {
        let _ = ureq::get(format!(
            "https://sctapi.ftqq.com/{}.send",
            config.push_ftqq_key
        ))
        .query("title", msg.title)
        .query("desp", msg.msg)
        .call()?;
    }
    Ok(())
}
