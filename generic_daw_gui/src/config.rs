use crate::{
	daw::{CONFIG_DIR, DATA_DIR},
	theme::Theme,
};
use generic_daw_core::{Device, DeviceId, HostId};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{
	fs::{read_to_string, write},
	io,
	num::NonZero,
	path::Path,
	sync::{Arc, LazyLock},
};

pub static CONFIG_PATH: LazyLock<Arc<Path>> =
	LazyLock::new(|| CONFIG_DIR.join("config.toml").into());

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Config {
	pub sample_paths: Vec<Arc<Path>>,
	pub clap_paths: Vec<Arc<Path>>,
	pub audio: Audio,
	pub autosave: Autosave,
	pub open_last_project: bool,
	pub scale_factor: f32,
	pub theme: Theme,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			sample_paths: vec![DATA_DIR.clone()],
			clap_paths: Vec::new(),
			audio: Audio::default(),
			autosave: Autosave::default(),
			open_last_project: false,
			scale_factor: 1.0,
			theme: Theme::CatppuccinFrappe,
		}
	}
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Audio {
	pub devices: Devices,
	pub sample_rate: Option<NonZero<u32>>,
	pub buffer_size: Option<NonZero<u32>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Devices {
	#[default]
	Default,
	WithHost {
		host: HostId,
		input: Option<Box<str>>,
		output: Option<Box<str>>,
	},
}

impl Devices {
	pub fn get_host(&self) -> Option<HostId> {
		match self {
			Self::Default => None,
			Self::WithHost { host, .. } => Some(*host),
		}
	}

	pub fn set_host(&mut self, host: Option<HostId>) {
		*self = host.map_or(Self::Default, |host| Self::WithHost {
			host,
			input: None,
			output: None,
		});
	}

	pub fn get_input(&self) -> Device {
		match self {
			Self::Default => Device::Default,
			Self::WithHost {
				host,
				input: Some(input),
				..
			} => Device::Specific(DeviceId::new(*host, input)),
			Self::WithHost { host, .. } => Device::DefaultForHost(*host),
		}
	}

	pub fn set_input(&mut self, input: Option<DeviceId>) {
		*self = match (self.clone(), input) {
			(Self::Default, None) => Self::Default,
			(Self::WithHost { host, output, .. }, None) => Self::WithHost {
				host,
				input: None,
				output,
			},
			(Self::Default, Some(input)) => Self::WithHost {
				host: input.host(),
				input: Some(input.id().into()),
				output: None,
			},
			(Self::WithHost { host, output, .. }, Some(input)) => Self::WithHost {
				output: output.filter(|_| host == input.host()),
				host: input.host(),
				input: Some(input.id().into()),
			},
		};
	}

	pub fn get_output(&self) -> Device {
		match self {
			Self::Default => Device::Default,
			Self::WithHost {
				host,
				output: Some(output),
				..
			} => Device::Specific(DeviceId::new(*host, output)),
			Self::WithHost { host, .. } => Device::DefaultForHost(*host),
		}
	}

	pub fn set_output(&mut self, output: Option<DeviceId>) {
		*self = match (self.clone(), output) {
			(Self::Default, None) => Self::Default,
			(Self::WithHost { host, input, .. }, None) => Self::WithHost {
				host,
				input,
				output: None,
			},
			(Self::Default, Some(output)) => Self::WithHost {
				host: output.host(),
				input: None,
				output: Some(output.id().into()),
			},
			(Self::WithHost { host, input, .. }, Some(output)) => Self::WithHost {
				input: input.filter(|_| host == output.host()),
				host: output.host(),
				output: Some(output.id().into()),
			},
		};
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

mod devices_serde {
	use serde::{Deserialize, Deserializer, Serialize, Serializer};

	#[derive(Deserialize, Serialize)]
	struct Devices {
		host: Box<str>,
		input: Option<Box<str>>,
		output: Option<Box<str>>,
	}

	impl Serialize for super::Devices {
		fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
			match self {
				Self::Default => None,
				Self::WithHost {
					host,
					input,
					output,
				} => Some(Devices {
					host: host.to_string().into(),
					input: input.clone(),
					output: output.clone(),
				}),
			}
			.serialize(serializer)
		}
	}

	impl<'de> Deserialize<'de> for super::Devices {
		fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
			Ok(match Option::<Devices>::deserialize(deserializer)? {
				None => Self::Default,
				Some(data) => Self::WithHost {
					host: data.host.parse().map_err(serde::de::Error::custom)?,
					input: data.input,
					output: data.output,
				},
			})
		}
	}
}
