mod config;
mod login;
mod push;
mod query;

use anyhow::Result;
use std::{env, path::PathBuf, process, str::FromStr, time::SystemTime};

use login::LoginSession;
use query::QuerySession;

fn load_config_path_from_args() -> Result<PathBuf> {
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        println!("Do not specify config path, sliently use .env");
        return Ok(PathBuf::from_str(".env")?);
    }
    if args.len() < 3 {
        eprintln!("Usage: {} -f <FILE_PATH>", args[0]);
        process::exit(1);
    }
    if &args[1] != "-f" {
        eprintln!("Error: Invalid flag. Expected -f.");
        eprintln!("Usage: {} --path <FILE_PATH>", args[0]);
        process::exit(1);
    }
    let path_str = &args[2];
    let config_path = PathBuf::from(path_str);
    if !config_path.exists() {
        eprintln!("Path verification: The file or directory does not exist.");
        process::exit(1);
    }
    Ok(config_path)
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
    let config_path = load_config_path_from_args()?;
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
