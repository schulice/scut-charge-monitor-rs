mod config;
mod login;
mod query;

use anyhow::Result;

use login::LoginSession;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let config = config::Config::read_from_env()?;
    let login_session = LoginSession::new();
    println!("[Login]: start process");
    login_session.process(&config).await?;
    println!("[Login]: finish process");
    Ok(())
}
