use log::warn;
use serde::{Deserialize, Serialize};
use std::{
	fs::{read_to_string, write},
	io,
	path::{Path, PathBuf},
	sync::{Arc, LazyLock},
};

pub static STATE_PATH: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
	dirs::state_dir()
		.or_else(dirs::data_dir)
		.or_else(|| {
			warn!("can't find the system's state/data dir!");
			None
		})
		.map(|path| path.join("generic_daw.toml"))
});

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
	pub last_project: Option<Arc<Path>>,
}

impl State {
	#[must_use]
	pub fn read() -> Self {
		let Some(state_path) = &*STATE_PATH else {
			return Self::default();
		};

		let config = read_to_string(state_path);

		let read =
			toml::from_str::<Self>(config.as_deref().unwrap_or_default()).unwrap_or_default();

		if config.is_err_and(|e| e.kind() == io::ErrorKind::NotFound) {
			read.write();
		}

		read
	}

	pub fn write(&self) {
		let Some(state_path) = &*STATE_PATH else {
			return;
		};

		write(state_path, toml::to_string(self).unwrap()).unwrap();
	}
}
