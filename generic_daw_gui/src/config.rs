use crate::{
	daw::{CONFIG_DIR, DATA_DIR},
	theme::Theme,
};
use generic_daw_core::{DeviceId, HostId};
use log::{info, warn};
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
	LazyLock::new(|| CONFIG_DIR.join("config.toml").into());

pub static DEFAULT_SAMPLE_PATHS: LazyLock<Box<[Arc<Path>]>> =
	LazyLock::new(|| boxed_slice![DATA_DIR.clone()]);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Config {
	pub sample_paths: Vec<Arc<Path>>,
	pub clap_paths: Vec<Arc<Path>>,
	pub devices: Devices,
	pub autosave: Autosave,
	pub open_last_project: bool,
	pub scale_factor: f32,
	pub theme: Theme,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			sample_paths: DEFAULT_SAMPLE_PATHS.clone().into_vec(),
			clap_paths: Vec::new(),
			devices: Devices::default(),
			autosave: Autosave::default(),
			open_last_project: false,
			scale_factor: 1.0,
			theme: Theme::CatppuccinFrappe,
		}
	}
}

impl Config {
	pub fn merge_with(&mut self, mut other: Self) {
		std::mem::swap(self, &mut other);
		self.devices.host = other.devices.host;
		self.devices.output = other.devices.output;
	}

	pub fn is_mergeable(&self, other: &Self) -> bool {
		self.devices.host == other.devices.host && self.devices.output == other.devices.output
	}
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Devices {
	#[serde(with = "option")]
	pub host: Option<HostId>,
	pub input: Device,
	pub output: Device,
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
	pub interval: NonZero<u16>,
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
		let read = match read_to_string(&*CONFIG_PATH) {
			Ok(read) => match toml::from_str(&read) {
				Ok(read) => read,
				Err(err) => {
					warn!("{err}");
					Self::default()
				}
			},
			Err(err) if err.kind() == io::ErrorKind::NotFound => {
				let read = Self::default();
				read.write();
				read
			}
			Err(err) => {
				warn!("{err}");
				Self::default()
			}
		};

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
	pub fn serialize<S: Serializer, T: ToString>(
		value: &Option<T>,
		serializer: S,
	) -> Result<S::Ok, S::Error> {
		match value {
			Some(v) => serializer.serialize_some(&v.to_string()),
			None => serializer.serialize_none(),
		}
	}

	pub fn deserialize<'de, D: Deserializer<'de>, T: FromStr<Err: Display>>(
		deserializer: D,
	) -> Result<Option<T>, D::Error> {
		match Option::<&str>::deserialize(deserializer)? {
			Some(s) => Ok(Some(T::from_str(s).map_err(serde::de::Error::custom)?)),
			None => Ok(None),
		}
	}
}
