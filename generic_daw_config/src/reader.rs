use crate::{error::ConfigError, Config};
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub struct Reader(Config);

impl Reader {
    pub fn from_str(s: &str) -> Result<Self, ConfigError> {
        Ok(Self(Config::from_str(s)?))
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path)?;
        Self::from_str(&contents)
    }

    #[must_use]
    pub fn config(&self) -> &Config {
        &self.0
    }

    #[must_use]
    pub fn into_config(self) -> Config {
        self.0
    }
}
