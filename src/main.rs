mod config;
mod login;
mod push;
mod query;

use anyhow::Result;
use clap::Parser;

use std::{path::PathBuf, time::SystemTime};

use login::LoginSession;
use query::QuerySession;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long, default_value = ".env")]
    config: Option<PathBuf>,
}

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} {} {}",
                humantime::format_rfc3339_seconds(SystemTime::now()),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        // .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = cli
        .config
        .ok_or_else(|| anyhow::anyhow!("config file argument error"))?;
    setup_logger()?;
    let config = config::Config::read_from_file(&config_path)?;
    let login_session = LoginSession::new()?;
    let access_token = login_session.process(&config)?;
    let login_agent = login_session.agent.clone();
    let query_session = QuerySession::from_agent(login_agent);
    query_session.perform_redirect(&access_token)?;
    let push_msg = query_session.get_charge_histroy(&config)?;
    let _ = push::push(&config, push_msg)?;
    Ok(())
}
