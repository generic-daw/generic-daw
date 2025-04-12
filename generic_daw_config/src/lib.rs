use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod error;
pub mod reader;
pub mod writer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub name: String,
    pub sample_rate: u32,
    pub buffer_size: u32,
}

impl Device {
    #[must_use]
    pub fn new(name: impl Into<String>, sample_rate: u32, buffer_size: u32) -> Self {
        Self {
            name: name.into(),
            sample_rate,
            buffer_size,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub sample_paths: Vec<String>,
    pub clap_paths: Vec<String>,
    pub input_device: Device,
    pub output_device: Device,
    pub autosave_interval: u32, // in seconds
}

impl Config {
    #[must_use]
    pub fn new(input_device: Device, output_device: Device, autosave_interval: u32) -> Self {
        Self {
            sample_paths: Vec::new(),
            clap_paths: Vec::new(),
            input_device,
            output_device,
            autosave_interval,
        }
    }

    #[must_use]
    pub fn sample_paths(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.sample_paths.iter().map(PathBuf::from)
    }

    #[must_use]
    pub fn clap_paths(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.clap_paths.iter().map(PathBuf::from)
    }

    pub fn to_string(&self) -> Result<String, error::ConfigError> {
        toml::to_string(self).map_err(|e| error::ConfigError::SerializationError(e.to_string()))
    }

    pub fn from_str(s: &str) -> Result<Self, error::ConfigError> {
        toml::from_str(s).map_err(|e| error::ConfigError::DeserializationError(e.to_string()))
    }
}
