use clack_extensions::gui::GuiApiType;
use std::{
	path::{Path, PathBuf},
	sync::{Arc, LazyLock},
};
use walkdir::{DirEntry, WalkDir};

mod audio_buffers;
mod audio_ports_config;
mod audio_processor;
mod audio_thread;
mod event_buffers;
mod event_ports_config;
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

pub use audio_processor::{ThreadPoolExecutor, ThreadPoolInjector};
pub use audio_thread::AudioThread;
pub use clack_extensions::{
	params::ParamInfoFlags, render::RenderMode, state_context::StateContextType, timer::TimerId,
};
pub use clack_host::{
	host::HostInfo,
	utils::{BeatTime, ClapId, Cookie, SecondsTime},
};
pub use main_thread::MainThreadMessage;
pub use plugin::Plugin;
pub use plugin_descriptor::PluginDescriptor;
pub use preset::Preset;
pub use size::Size;

const API_TYPE: GuiApiType<'_> = GuiApiType::default_for_current_platform().unwrap();

pub static DEFAULT_CLAP_PATHS: LazyLock<Box<[Arc<Path>]>> = LazyLock::new(|| {
	let mut paths = Vec::new();

	if cfg!(windows) {
		if let Some(path) = std::env::var_os("COMMONPROGRAMFILES").map(PathBuf::from) {
			paths.push(path.join("CLAP").into());
		}

		if let Some(path) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
			paths.push(path.join("Programs\\Common\\CLAP").into());
		}
	} else if cfg!(target_os = "macos") {
		paths.push(Path::new("/Library/Audio/Plug-Ins/CLAP").into());

		if let Some(path) = std::env::var_os("HOME").map(PathBuf::from) {
			paths.push(path.join("Library/Audio/Plug-Ins/CLAP").into());
		}
	} else if cfg!(unix) {
		if let Some(path) = std::env::var_os("HOME").map(PathBuf::from) {
			paths.push(path.join(".clap").into());
		}

		paths.push(Path::new("/usr/lib/clap").into());

		if std::fs::symlink_metadata("/usr/lib64").is_ok_and(|metadata| {
			!metadata.is_symlink()
				|| std::fs::canonicalize("/usr/lib64").is_ok_and(|path| &path != "/usr/lib")
		}) {
			paths.push(Path::new("/usr/lib64/clap").into());
		}
	}

	if let Some(clap_path) = std::env::var_os("CLAP_PATH") {
		paths.extend(std::env::split_paths(&clap_path).map(Arc::from));
	}

	paths.into_boxed_slice()
});

pub fn find_plugin_paths(
	paths: impl IntoIterator<Item: AsRef<Path>>,
) -> impl Iterator<Item = PathBuf> {
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
		.map(DirEntry::into_path)
}
