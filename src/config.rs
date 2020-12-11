//! The C19 configuration.
//!
//! The configuration automatically initializes and holds the instances of all layers (Agent, State
//! and Connection).
//!
//! A YAML formatted configuration file is automatically loaded when the c19 process
//! starts. The path to the configuration file is specified using the --config flag when running
//! the process.
//!
//! # Examples:
//!
//! The configuration it based on the different layers chosen. Evey layer implementation has 
//! its own configuration.
//!
//! ## Here's a small example for a configuration file:
//! ```
//! version: 0.1
//! spec:
//!   agent:
//!     kind: Default
//!     port: 3097
//!   state:
//!     kind: Default
//!     ttl: null
//!     purge_interval: 10000
//!     data_seeder:
//!       kind: File
//!       filename: data.json
//!   connection:
//!     kind: Default
//!     push_interval: 1000
//!     pull_interval: 60000
//!     force_publish: 0.1
//!     port: 4097
//!     target_port: 4098
//!     r0: 6
//!     timeout: 5000
//!     peer_provider:
//!       kind: Static
//!       peers:
//!         - 127.0.0.1
//! ```
//!
//! The configuration file is devided into three parts, one for each layer. The configuration
//! for each layer is specific to that layer based on its kind. In this example you can see that
//! the configuration will load the Default agent, Default state and Default connection and within
//! the connection layer it will load the static peer provider.
//!
//! See the documentation for each layer implementation to find out what available configuration
//! settings there are.

use crate::agent;
use crate::connection;
use crate::state;

use clap::ArgMatches;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fs;

const DEFAULT_CONFIG_FILE: &str = "config.yml";

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub version: String,
    pub spec: Spec,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Spec {
    pub agent: Box<dyn agent::Agent>,
    pub state: Box<dyn state::State>,
    pub connection: Box<dyn connection::Connection>,
}

/// Returns the configuration after dynanmically loading all layers.
///
/// Takes command line arguments, loads the YAML configuration file and initializes all layer
/// objects.
pub fn new(matches: &ArgMatches) -> Result<Config, Box<dyn Error>> {
    let config_file = matches.value_of("config").unwrap_or(DEFAULT_CONFIG_FILE);
    let config = fs::read_to_string(config_file)?;

    let config: Config = serde_yaml::from_str(&config)?;

    Ok(config)
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = serde_yaml::to_string(self).unwrap_or("Failed to parse config to yaml".to_string());

        f.write_str(s.as_ref())
    }
}
