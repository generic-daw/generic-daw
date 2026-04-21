use crate::daw::STATE_DIR;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{
	fs::{read_to_string, write},
	io,
	path::Path,
	sync::{Arc, LazyLock},
};

pub static STATE_PATH: LazyLock<Arc<Path>> = LazyLock::new(|| STATE_DIR.join("state.toml").into());

pub const DEFAULT_SPLIT_POSITION: f32 = 300.0;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
	pub last_project: Option<Arc<Path>>,
	pub file_tree_split_at: f32,
	pub plugins_panel_split_at: f32,
	pub show_seconds: bool,
	pub metronome: bool,
	pub autoscroll: bool,
}

impl Default for State {
	fn default() -> Self {
		Self {
			last_project: None,
			file_tree_split_at: DEFAULT_SPLIT_POSITION,
			plugins_panel_split_at: DEFAULT_SPLIT_POSITION,
			show_seconds: false,
			metronome: false,
			autoscroll: false,
		}
	}
}

impl State {
	pub fn read() -> Self {
		let read = match read_to_string(&*STATE_PATH) {
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

		info!("loaded state {read:#?}");

		read
	}

	pub fn write(&self) {
		write(&*STATE_PATH, toml::to_string(self).unwrap()).unwrap();
	}
}
