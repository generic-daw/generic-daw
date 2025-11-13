use crate::daw::DEFAULT_SPLIT_POSITION;
use log::info;
use serde::{Deserialize, Serialize};
use std::{
	fs::{read_to_string, write},
	io,
	path::Path,
	sync::{Arc, LazyLock},
};

pub static STATE_PATH: LazyLock<Arc<Path>> = LazyLock::new(|| {
	dirs::state_dir()
		.or_else(dirs::data_dir)
		.unwrap()
		.join("generic_daw.toml")
		.into()
});

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
	pub last_project: Option<Arc<Path>>,
	pub file_tree_split_at: f32,
	pub plugins_panel_split_at: f32,
}

impl Default for State {
	fn default() -> Self {
		Self {
			last_project: None,
			file_tree_split_at: DEFAULT_SPLIT_POSITION,
			plugins_panel_split_at: DEFAULT_SPLIT_POSITION,
		}
	}
}

impl State {
	pub fn read() -> Self {
		let config = read_to_string(&*STATE_PATH);

		let read =
			toml::from_str::<Self>(config.as_deref().unwrap_or_default()).unwrap_or_default();

		if config.is_err_and(|e| e.kind() == io::ErrorKind::NotFound) {
			read.write();
		}

		info!("loaded state {read:#?}");

		read
	}

	pub fn write(&self) {
		write(&*STATE_PATH, toml::to_string(self).unwrap()).unwrap();
	}
}
