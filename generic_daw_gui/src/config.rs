use crate::{arrangement_view::DATA_DIR, theme::Theme};
use generic_daw_core::clap_host::DEFAULT_CLAP_PATHS;
use log::info;
use serde::{Deserialize, Serialize};
use std::{
	fs::{read_to_string, write},
	io,
	num::NonZero,
	path::Path,
	sync::{Arc, LazyLock},
};

pub static CONFIG_PATH: LazyLock<Arc<Path>> =
	LazyLock::new(|| dirs::config_dir().unwrap().join("generic_daw.toml").into());

pub static DEFAULT_SAMPLE_PATHS: LazyLock<Vec<Arc<Path>>> =
	LazyLock::new(|| vec![dirs::home_dir().unwrap().into(), DATA_DIR.clone()]);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Config {
	pub sample_paths: Vec<Arc<Path>>,
	pub clap_paths: Vec<Arc<Path>>,
	pub input_device: Device,
	pub output_device: Device,
	pub autosave: Autosave,
	pub open_last_project: bool,
	pub app_scale_factor: f32,
	pub plugin_scale_factor: Option<f32>,
	pub theme: Theme,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			sample_paths: DEFAULT_SAMPLE_PATHS.clone(),
			clap_paths: DEFAULT_CLAP_PATHS.clone(),
			input_device: Device::default(),
			output_device: Device::default(),
			autosave: Autosave::default(),
			open_last_project: false,
			app_scale_factor: 1.0,
			plugin_scale_factor: None,
			theme: Theme::default(),
		}
	}
}

impl Config {
	pub fn merge_with(&mut self, mut other: Self) {
		std::mem::swap(self, &mut other);
		self.output_device = other.output_device;
	}

	pub fn is_mergeable(&self, other: &Self) -> bool {
		self.output_device == other.output_device
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Device {
	pub name: Option<Arc<str>>,
	pub sample_rate: NonZero<u32>,
	pub buffer_size: Option<NonZero<u32>>,
}

impl Default for Device {
	fn default() -> Self {
		Self {
			name: None,
			sample_rate: NonZero::new(44100).unwrap(),
			buffer_size: None,
		}
	}
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Autosave {
	pub enabled: bool,
	pub interval: NonZero<u64>,
}

impl Default for Autosave {
	fn default() -> Self {
		Self {
			enabled: true,
			interval: NonZero::new(300).unwrap(),
		}
	}
}

impl Config {
	pub fn read() -> Self {
		let config = read_to_string(&*CONFIG_PATH);

		let read =
			toml::from_str::<Self>(config.as_deref().unwrap_or_default()).unwrap_or_default();

		if config.is_err_and(|e| e.kind() == io::ErrorKind::NotFound) {
			read.write();
		}

		info!("loaded config {read:#?}");

		read
	}

	pub fn write(&self) {
		write(&*CONFIG_PATH, toml::to_string(self).unwrap()).unwrap();
	}
}
