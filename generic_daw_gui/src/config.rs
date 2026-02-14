use crate::{arrangement_view::DATA_DIR, theme::Theme};
use generic_daw_core::DeviceId;
use log::info;
use serde::{Deserialize, Serialize};
use std::{
	fs::{read_to_string, write},
	io,
	num::NonZero,
	path::Path,
	sync::{Arc, LazyLock},
};
use utils::boxed_slice;

pub static CONFIG_PATH: LazyLock<Arc<Path>> =
	LazyLock::new(|| dirs::config_dir().unwrap().join("generic_daw.toml").into());

pub static DEFAULT_SAMPLE_PATHS: LazyLock<Box<[Arc<Path>]>> =
	LazyLock::new(|| boxed_slice![dirs::home_dir().unwrap().into(), DATA_DIR.clone()]);

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
			sample_paths: DEFAULT_SAMPLE_PATHS.clone().into_vec(),
			clap_paths: Vec::new(),
			input_device: Device::default(),
			output_device: Device::default(),
			autosave: Autosave::default(),
			open_last_project: false,
			app_scale_factor: 1.0,
			plugin_scale_factor: None,
			theme: Theme::CatppuccinFrappe,
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
	#[serde(with = "option")]
	pub id: Option<DeviceId>,
	pub sample_rate: NonZero<u32>,
	pub buffer_size: Option<NonZero<u32>>,
}

impl Default for Device {
	fn default() -> Self {
		Self {
			id: None,
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

mod option {
	use serde::{Deserialize as _, Deserializer, Serializer};
	use std::{fmt::Display, str::FromStr};

	#[expect(clippy::ref_option)]
	pub fn serialize<S, T>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
		T: ToString,
	{
		match value {
			Some(v) => serializer.serialize_some(&v.to_string()),
			None => serializer.serialize_none(),
		}
	}

	pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
	where
		D: Deserializer<'de>,
		T: FromStr,
		T::Err: Display,
	{
		match Option::<&str>::deserialize(deserializer)? {
			Some(s) => Ok(Some(T::from_str(s).map_err(serde::de::Error::custom)?)),
			None => Ok(None),
		}
	}
}
