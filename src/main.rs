use c19::config;
use clap::App;
use log::{error, info};
use std::process;

use c19;

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = App::new("The C19 Protocol")
        .version("0.1.0")
        .author("Chen Fisher")
        .about("A variant of the gossip protocol. Allows a group of servies to agree on a service-wide state")
        .arg("-c, --config=[FILE] 'Set the path to a c19 config file'")
        .get_matches();

    // load config
    let config = config::new(&args).unwrap_or_else(|err| {
        error!("Failed to load config file; ({})", err);
        process::exit(1);
    });

    info!("Config: {}", config);

    // ...and run
    if let Err(e) = c19::run(config).await {
        error!("Failed to run c19: {}", e);
        process::exit(1);
    }
}
