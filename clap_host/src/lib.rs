use clack_extensions::gui::GuiApiType;
use log::warn;
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::{Arc, LazyLock},
};
use utils::NoDebug;
use walkdir::WalkDir;

mod audio_buffers;
mod audio_ports_config;
mod audio_processor;
mod audio_thread;
mod event_buffers;
mod event_impl;
pub mod events;
mod gui;
mod host;
mod main_thread;
mod param;
mod plugin;
mod plugin_descriptor;
mod preset;
mod shared;
mod size;

pub use audio_processor::AudioProcessor;
#[cfg(unix)]
pub use clack_extensions::posix_fd::FdFlags;
pub use clack_extensions::{
	params::{ParamInfoFlags, ParamRescanFlags},
	state_context::StateContextType,
	timer::TimerId,
};
pub use clack_host::{
	bundle::PluginBundle,
	host::HostInfo,
	utils::{ClapId, Cookie},
};
pub use event_impl::EventImpl;
pub use main_thread::MainThreadMessage;
pub use plugin::Plugin;
pub use plugin_descriptor::PluginDescriptor;
pub use size::Size;

const API_TYPE: GuiApiType<'_> = GuiApiType::default_for_current_platform().unwrap();

pub static DEFAULT_CLAP_PATHS: LazyLock<Box<[Arc<Path>]>> = LazyLock::new(|| {
	let mut paths = Vec::new();

	if cfg!(target_os = "windows") {
		if let Some(path) = std::env::var_os("COMMONPROGRAMFILES").map(PathBuf::from) {
			paths.push(path.join("CLAP").into());
		}

		if let Some(path) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
			paths.push(path.join("Programs\\Common\\CLAP").into());
		}
	}

	if cfg!(target_os = "macos") {
		paths.push(Path::new("/Library/Audio/Plug-Ins/CLAP").into());

		if let Some(path) = std::env::var_os("HOME").map(PathBuf::from) {
			paths.push(path.join("Library/Audio/Plug-Ins/CLAP").into());
		}
	} else if cfg!(unix) {
		if let Some(path) = std::env::var_os("HOME").map(PathBuf::from) {
			paths.push(path.join(".clap").into());
		}

		paths.push(Path::new("/usr/lib/clap").into());
		paths.push(Path::new("/usr/lib64/clap").into());
	}

	if let Some(clap_path) = std::env::var_os("CLAP_PATH") {
		paths.extend(std::env::split_paths(&clap_path).map(Arc::from));
	}

	paths.into_boxed_slice()
});

#[must_use]
pub fn get_installed_plugins(
	paths: impl IntoIterator<Item: AsRef<Path>>,
) -> HashMap<PluginDescriptor, NoDebug<PluginBundle>> {
	let mut bundles = HashMap::new();

	paths
		.into_iter()
		.flat_map(|path| WalkDir::new(path).follow_links(true))
		.flatten()
		.filter(|dir_entry| {
			if cfg!(target_os = "macos") {
				dir_entry.file_type().is_dir()
			} else {
				dir_entry.file_type().is_file()
			}
		})
		.filter(|dir_entry| {
			dir_entry
				.path()
				.extension()
				.is_some_and(|ext| ext == "clap")
		})
		.filter_map(|dir_entry| {
			// SAFETY:
			// Loading an external library object file is inherently unsafe.
			unsafe { PluginBundle::load(dir_entry.path()) }
				.inspect_err(|err| warn!("{}: {err}", dir_entry.path().display()))
				.ok()
		})
		.for_each(|bundle| {
			if let Some(factory) = bundle.get_plugin_factory() {
				factory
					.plugin_descriptors()
					.filter_map(|d| d.try_into().ok())
					.for_each(|d| {
						bundles.insert(d, bundle.clone().into());
					});
			}
		});

	bundles
}
