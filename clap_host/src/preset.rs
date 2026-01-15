use crate::{PluginDescriptor, host::Host};
use clack_extensions::preset_discovery::prelude::*;
use clack_host::prelude::*;
use log::{log_enabled, warn};
use std::{
	ffi::{CStr, CString},
	fmt::Write as _,
};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct Preset {
	pub name: Box<str>,
	pub location: MyLocation,
	pub load_key: Option<Box<CStr>>,
}

impl Preset {
	pub fn all(
		plugin: &PluginInstance<Host>,
		bundle: &PluginBundle,
		descriptor: &PluginDescriptor,
		host: &HostInfo,
	) -> Option<Box<[Self]>> {
		plugin.access_shared_handler(|s| s.ext.preset_load.get())?;

		let preset_discovery_factory = bundle.get_factory::<PresetDiscoveryFactory<'_>>()?;

		let mut cached_indexer = Indexer::default();
		let mut presets = Vec::new();

		for provider_descriptor in preset_discovery_factory.provider_descriptors() {
			let Some(provider_id) = provider_descriptor.id() else {
				continue;
			};

			cached_indexer.match_all = false;
			cached_indexer.file_types.clear();
			cached_indexer.locations.clear();
			let Ok(mut provider) =
				Provider::instantiate(&mut cached_indexer, bundle, provider_id, host)
			else {
				continue;
			};

			let indexer = std::mem::take(*provider.indexer_mut());

			for location in &indexer.locations {
				let mut metadata_receiver = MetadataReceiver {
					presets: &mut presets,
					current_preset: None,
					descriptor,
					location,
				};

				match location {
					MyLocation::Plugin => {
						provider.get_metadata(location.into(), &mut metadata_receiver);
					}
					MyLocation::File(path) => {
						WalkDir::new(&*path.to_string_lossy())
							.follow_links(true)
							.into_iter()
							.filter_map(Result::ok)
							.filter(|dir_entry| dir_entry.file_type().is_file())
							.filter(|dir_entry| {
								indexer.match_all
									|| dir_entry.path().extension().is_some_and(|extension| {
										indexer
											.file_types
											.iter()
											.any(|file_type| **file_type == *extension)
									})
							})
							.for_each(|dir_entry| {
								if let Some(path) = dir_entry.path().to_str()
									&& let Ok(path) = CString::new(path)
								{
									provider.get_metadata(
										Location::File { path: &path },
										&mut metadata_receiver,
									);
								}
							});
					}
				}
			}

			drop(provider);
			cached_indexer = indexer;
		}

		Some(presets.into_boxed_slice())
	}
}

#[derive(Clone, Debug)]
pub enum MyLocation {
	Plugin,
	File(Box<CStr>),
}

impl From<Location<'_>> for MyLocation {
	fn from(value: Location<'_>) -> Self {
		match value {
			Location::File { path } => Self::File(path.into()),
			Location::Plugin => Self::Plugin,
		}
	}
}

impl<'a> From<&'a MyLocation> for Location<'a> {
	fn from(value: &'a MyLocation) -> Self {
		match value {
			MyLocation::File(path) => Self::File { path },
			MyLocation::Plugin => Self::Plugin,
		}
	}
}

#[derive(Default)]
struct Indexer {
	match_all: bool,
	file_types: Vec<Box<str>>,
	locations: Vec<MyLocation>,
}

impl IndexerImpl for &mut Indexer {
	fn declare_filetype(&mut self, file_type: FileType<'_>) -> Result<(), HostError> {
		if !self.match_all
			&& let Some(extension) = file_type.file_extension
			&& !extension.is_empty()
		{
			self.file_types.push(extension.to_string_lossy().into());
		} else {
			self.match_all = true;
		}

		Ok(())
	}

	fn declare_location(&mut self, location: LocationInfo<'_>) -> Result<(), HostError> {
		self.locations.push(location.location.into());
		Ok(())
	}

	fn declare_soundpack(&mut self, _soundpack: Soundpack<'_>) -> Result<(), HostError> {
		Ok(())
	}
}

struct MetadataReceiver<'a> {
	presets: &'a mut Vec<Preset>,
	current_preset: Option<Preset>,
	descriptor: &'a PluginDescriptor,
	location: &'a MyLocation,
}

impl Drop for MetadataReceiver<'_> {
	fn drop(&mut self) {
		self.presets.extend(self.current_preset.take());
	}
}

impl MetadataReceiverImpl for MetadataReceiver<'_> {
	fn on_error(&mut self, error_code: i32, error_message: Option<&CStr>) {
		if !log_enabled!(log::Level::Warn) {
			return;
		}

		let mut message = String::new();

		if let Some(preset) = &self.current_preset {
			write!(message, "{}: {}", &self.descriptor, &preset.name).unwrap();
		} else {
			write!(message, "{}: preset error", &self.descriptor).unwrap();
		}

		if let Some(error_message) = error_message {
			write!(message, ": {error_message:?}").unwrap();

			if error_code != 0 {
				write!(message, " (os error {error_code})").unwrap();
			}
		} else if error_code != 0 {
			write!(message, ": os error {error_code}").unwrap();
		}

		warn!("{message}");
	}

	fn begin_preset(
		&mut self,
		name: Option<&CStr>,
		load_key: Option<&CStr>,
	) -> Result<(), HostError> {
		self.presets.extend(self.current_preset.take());

		if let Some(name) = name {
			self.current_preset = Some(Preset {
				name: Box::from(name.to_str()?),
				location: self.location.clone(),
				load_key: load_key.map(Box::from),
			});
		}

		Ok(())
	}

	fn add_plugin_id(&mut self, _plugin_id: UniversalPluginId<'_>) {}

	fn set_soundpack_id(&mut self, _soundpack_id: &CStr) {}

	fn set_flags(&mut self, _flags: Flags) {}

	fn add_creator(&mut self, _creator: &CStr) {}

	fn set_description(&mut self, _description: &CStr) {}

	fn set_timestamps(
		&mut self,
		_creation_time: Option<Timestamp>,
		_modification_time: Option<Timestamp>,
	) {
	}

	fn add_feature(&mut self, _feature: &CStr) {}

	fn add_extra_info(&mut self, _key: &CStr, _value: &CStr) {}
}
