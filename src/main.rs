mod config;
mod login;
mod query;

use anyhow::Result;
use log::info;
use std::{path::Path, time::SystemTime};

use login::LoginSession;

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
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        // .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}

fn main() -> Result<()> {
    setup_logger()?;
    let config = config::Config::read_from_envfile(Path::new("./.env"))?;
    let login_session = LoginSession::new()?;
    info!("[Login]: start process");
    login_session.process(&config)?;
    info!("[Login]: finish process");
    Ok(())
}
