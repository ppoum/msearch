use std::fs;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::RwLock;
use lazy_static::lazy_static;
use yaml_rust::YamlLoader;

struct Config {
    loaded: bool,
    dispatcher_base: String
}

lazy_static! {
    static ref CONFIG: RwLock<Config> = RwLock::new(Config {
        loaded: false,
        dispatcher_base: String::new()
    });
}

pub fn load_config(path: &str) -> Result<(), Box<dyn Error>> {
    let mut config = CONFIG.write().unwrap();
    let yaml = YamlLoader::load_from_str(&fs::read_to_string(path)?)?;
    if yaml.is_empty() {
        return Err(Box::new(ConfigParseError::new("File does not contain valid yaml syntax.")));
    }

    let yaml = yaml[0].clone();

    config.dispatcher_base = String::from(yaml["dispatcher_base"].as_str()
        .ok_or_else(|| ConfigParseError::new("dispatcher_base field is missing or invalid."))?);

    config.loaded = true;
    Ok(())
}

//
// GETTERS
//

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