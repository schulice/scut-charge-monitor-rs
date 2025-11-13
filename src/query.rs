use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use ureq::Agent;

use crate::config::{self, Config};
use crate::push::PushMessage;

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ElectricityUsage {
    pub used_ele_id: i64,
    pub left_ele_quantity: String,
    pub left_free_ele_quantity: Option<String>,
    pub daily_used_ele_quantity: String,
    pub time: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GetChargeHistoryResp {
    pub status_code: String,
    pub message: Option<String>,
    pub result_object: Vec<ElectricityUsage>,
}

pub struct QuerySession {
    agent: Agent,
}

impl QuerySession {
    pub fn from_agent(agent: Agent) -> Self {
        Self { agent }
    }
}

impl QuerySession {
    pub fn perform_redirect(&self, access_token: &str) -> Result<()> {
        let req = json!(
            {
                "appId": "360",
                "loginFrom": "h5",
                "synAccessSource": "h5",
                "synjones-auth": access_token,
                "type": "app"
            }
        );
        // Now we just get the url to auto set bear
        let _ = self
            .agent
            .get(config::URL_REDIRECT)
            .force_send_body()
            .send_json(&req)?;
        Ok(())
    }

    pub fn get_charge_histroy(&self, config: &Config) -> Result<PushMessage> {
        // Need to get first to enter query domain
        let _ = self.agent.get(config::URL_CHARGE_HISTORY).call()?;
        let mut resp = self
            .agent
            .get(config::URL_CHARGE_HISTORY)
            .query("idCode", 1001.to_string())
            .call()?;
        let resp: GetChargeHistoryResp = resp.body_mut().read_json()?;
        Self::analyze(&resp.result_object, config)
    }

    fn analyze(usage: &[ElectricityUsage], config: &Config) -> Result<PushMessage> {
        let left = usage
            .first()
            .ok_or_else(|| anyhow::anyhow!("ElectricityUsage with size zero"))?
            .left_ele_quantity
            .clone()
            .parse::<f64>()?;
        let current_week: Vec<f64> = usage
            .iter()
            .take(7)
            .map(|x| x.daily_used_ele_quantity.parse::<f64>().unwrap_or(0.0))
            .filter(|x| x.is_sign_positive())
            .take(7)
            .collect();
        let avg = current_week.iter().sum::<f64>() / current_week.len() as f64;

        let mut is_warning = avg > left;
        if config.electricity_alert_threshold != 0 {
            is_warning |= left < config.electricity_alert_threshold as f64;
        }
        let title = if is_warning {
            "Warning Electricity Report"
        } else {
            "Normal Electricity Usage Report"
        };
        let msg = format!("Left: {:.2}, WeekAvg: {:.2}", left, avg);
        Ok(PushMessage {
            is_warning,
            title: title.to_string(),
            msg,
        })
    }
}
