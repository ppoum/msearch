extern crate yaml_rust;

use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::fs;
use std::sync::RwLock;
use std::time::Duration;
use lazy_static::lazy_static;
use yaml_rust::YamlLoader;

use crate::Result;

struct Config {
    loaded: bool,
    receive_timeout: Duration,
    job_size: u64,
    send_rate: u64,
    dispatcher_base: String,
}

lazy_static! {
    static ref CONFIG: RwLock<Config> = RwLock::new(Config {
        loaded: false,
        receive_timeout: Duration::ZERO,
        job_size: 0,
        send_rate: 0,
        dispatcher_base: String::new()
    });
}

pub fn load_config(path: &str) -> Result<()> {
    let mut config = CONFIG.write().unwrap();

    let yaml = YamlLoader::load_from_str(fs::read_to_string(path)?.as_str())?[0].clone();

    let receive_timeout = yaml["stop_timeout"].as_i64()
        .ok_or_else(|| ConfigParseError::new("stop_timeout field missing or invalid."))?;
    config.receive_timeout = Duration::from_secs(receive_timeout as u64);

    config.job_size = yaml["job_size"].as_i64()
        .ok_or_else(|| ConfigParseError::new("job_size field is missing or invalid."))? as u64;

    config.send_rate = yaml["send_rate"].as_i64()
        .ok_or_else(|| ConfigParseError::new("send_rate field is missing or invalid."))? as u64;

    config.dispatcher_base = String::from(yaml["dispatcher_base"].as_str()
        .ok_or_else(|| ConfigParseError::new("dispatcher_base field is missing or invalid."))?);

    // Config file was valid
    config.loaded = true;
    Ok(())
}

//
// GETTERS
//

pub fn get_receive_timeout() -> Duration {
    let config = CONFIG.read().unwrap();
    assert!(config.loaded, "Tried to access config field before loading the file.");
    config.receive_timeout
}

pub fn get_job_size() -> u64 {
    let config = CONFIG.read().unwrap();
    assert!(config.loaded, "Tried to access config field before loading the file.");
    config.job_size
}

pub fn get_send_rate() -> u64 {
    let config = CONFIG.read().unwrap();
    assert!(config.loaded, "Tried to access config field before loading the file.");
    config.send_rate
}

pub fn get_dispatcher_base() -> String {
    let config = CONFIG.read().unwrap();
    assert!(config.loaded, "Tried to access config field before loading the file.");
    config.dispatcher_base.clone()
}

#[derive(Debug, Clone, )]
struct ConfigParseError {
    msg: String,
}

impl ConfigParseError {
    pub fn new(msg: &str) -> ConfigParseError {
        ConfigParseError { msg: String::from(msg) }
    }
}

impl Display for ConfigParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error parsing config: {}", self.msg)
    }
}

impl Error for ConfigParseError {}