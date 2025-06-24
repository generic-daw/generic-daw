use crate::theme::Theme;
use generic_daw_core::clap_host::default_clap_paths;
use serde::{Deserialize, Serialize};
use std::{
    fs::{read_to_string, write},
    io,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

pub static CONFIG_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| dirs::config_dir().unwrap().join("generic_daw.toml"));

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub sample_paths: Vec<Arc<Path>>,
    pub clap_paths: Vec<Arc<Path>>,
    pub input_device: Device,
    pub output_device: Device,
    pub autosave: Autosave,
    pub open_last_project: bool,
    pub theme: Theme,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sample_paths: default_sample_paths(),
            clap_paths: default_clap_paths(),
            input_device: Device::default(),
            output_device: Device::default(),
            autosave: Autosave::default(),
            open_last_project: false,
            theme: Theme::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Device {
    pub name: Option<String>,
    pub sample_rate: Option<u32>,
    pub buffer_size: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Autosave {
    pub enabled: bool,
    pub interval: u64,
}

impl Default for Autosave {
    fn default() -> Self {
        Self {
            enabled: false,
            interval: 600,
        }
    }
}

fn default_sample_paths() -> Vec<Arc<Path>> {
    vec![
        dirs::home_dir().unwrap().into(),
        dirs::data_dir().unwrap().join("Generic Daw").into(),
    ]
}

impl Config {
    #[must_use]
    pub fn read() -> Self {
        let config = read_to_string(&*CONFIG_PATH);

        let read =
            toml::from_str::<Self>(config.as_deref().unwrap_or_default()).unwrap_or_default();

        if config.is_err_and(|e| e.kind() == io::ErrorKind::NotFound) {
            read.write();
        }

        read
    }

    pub fn write(&self) {
        write(&*CONFIG_PATH, toml::to_string(self).unwrap()).unwrap();
    }
}
