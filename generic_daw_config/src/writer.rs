use crate::{error::ConfigError, Config, Device};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Writer(Config);

impl Writer {
    #[must_use]
    pub fn new(
        input_device: Device,
        output_device: Device,
        autosave_interval_seconds: u32,
    ) -> Self {
        Self(Config::new(
            input_device,
            output_device,
            autosave_interval_seconds,
        ))
    }

    #[must_use]
    pub fn config(&self) -> &Config {
        &self.0
    }

    #[must_use]
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.0
    }

    pub fn set_input_device(&mut self, device: Device) {
        self.0.input_device = device;
    }

    pub fn set_output_device(&mut self, device: Device) {
        self.0.output_device = device;
    }

    pub fn set_autosave_interval(&mut self, seconds: u32) {
        self.0.autosave_interval = seconds;
    }

    pub fn add_sample_path(&mut self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let path = path.as_ref();
        self.check_path_containment(path, &self.0.sample_paths)?;
        self.0.sample_paths.push(path.to_string_lossy().to_string());
        Ok(())
    }

    pub fn add_clap_path(&mut self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let path = path.as_ref();
        self.check_path_containment(path, &self.0.clap_paths)?;
        self.0.clap_paths.push(path.to_string_lossy().to_string());
        Ok(())
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let toml_string = self.0.to_string()?;
        fs::write(path, toml_string)?;
        Ok(())
    }
    pub fn to_string(&self) -> Result<String, ConfigError> {
        self.0.to_string()
    }

    #[must_use]
    pub fn into_config(self) -> Config {
        self.0
    }

    fn check_path_containment(&self, new_path: &Path, paths: &[String]) -> Result<(), ConfigError> {
        let new_path_buf = PathBuf::from(new_path);

        for existing_path in paths {
            let existing = PathBuf::from(existing_path);

            if new_path.starts_with(&existing) {
                return Err(ConfigError::PathContainment(
                    new_path_buf.clone(),
                    existing.clone(),
                ));
            }

            if existing.starts_with(new_path) {
                return Err(ConfigError::PathContainment(
                    existing.clone(),
                    new_path_buf.clone(),
                ));
            }
        }

        Ok(())
    }
}
