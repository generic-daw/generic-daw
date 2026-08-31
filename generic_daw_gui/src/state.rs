use crate::daw::STATE_DIR;
use generic_daw_core::{
	Transport,
	time::{BeatTime, SecondsTime},
};
use iced::{Vector, keyboard::Modifiers};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{
	fs::{read_to_string, write},
	io,
	path::Path,
	sync::{Arc, LazyLock},
};

pub static STATE_PATH: LazyLock<Arc<Path>> = LazyLock::new(|| STATE_DIR.join("state.toml").into());

pub const MIN_VERTICAL_SPLIT_AT: f32 = 300.0;
pub const MIN_HORIZONTAL_SPLIT_AT: f32 = 200.0;
pub const DEFAULT_VERTICAL_SPLIT_AT: f32 = 400.0;
pub const DEFAULT_HORIZONTAL_SPLIT_AT: f32 = 300.0;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
	pub last_project: Option<Arc<Path>>,
	pub file_tree_split_at: f32,
	pub plugins_pane_split_at: f32,
	pub bottom_pane_split_at: f32,
	pub show_seconds: bool,
	pub metronome: bool,
	pub autoscroll: bool,
	pub grid: Grid,
}

impl Default for State {
	fn default() -> Self {
		Self {
			last_project: None,
			file_tree_split_at: DEFAULT_HORIZONTAL_SPLIT_AT,
			plugins_pane_split_at: DEFAULT_HORIZONTAL_SPLIT_AT,
			bottom_pane_split_at: DEFAULT_VERTICAL_SPLIT_AT,
			show_seconds: false,
			metronome: false,
			autoscroll: false,
			grid: Grid::default(),
		}
	}
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Grid {
	pub size: f32,
	pub triplets: bool,
	pub enabled: bool,
}

impl Grid {
	pub fn maybe_snap<T>(self, t: T, modifiers: Modifiers, f: impl FnOnce(T) -> T) -> T {
		if self.enabled == modifiers.alt() {
			t
		} else {
			f(t)
		}
	}

	pub fn beats_snap_step(self, scale: Vector, transport: &Transport) -> BeatTime {
		let size = self.size + scale.x + (f32::from(transport.bpm.get()) / 60.0).log2();
		if size > 0.0 {
			BeatTime::new(u64::from(transport.numerator.get()), 0)
				<< (size - f32::from(transport.numerator.get()).log2()).ceil() as u8
		} else if self.triplets {
			let size = size - f32::log2(2.0 / 3.0);
			(if size > 0.0 {
				BeatTime::BEAT << size.ceil() as u8
			} else {
				BeatTime::BEAT >> -size.max(-9.0) as u8
			}) * 2 / 3
		} else {
			BeatTime::BEAT >> -size.max(-9.0) as u8
		}
	}

	pub fn seconds_snap_step(self, scale: Vector) -> SecondsTime {
		let size = self.size + scale.x;
		if size > 0.0 {
			let seconds = [2, 3, 4, 5, 6, 10, 12, 15, 20, 30]
				.into_iter()
				.find(|&step| size < f32::from(step).log2())
				.unwrap_or(60u8);
			SecondsTime::new(seconds.into(), 0)
		} else {
			SecondsTime::SECOND >> -size.max(-9.0) as u8
		}
	}
}

impl Default for Grid {
	fn default() -> Self {
		Self {
			size: 2.5,
			triplets: false,
			enabled: true,
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
