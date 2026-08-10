use generic_daw_core::{
	MidiKey, Transport,
	time::{BeatTime, SecondsTime},
};
use iced::Vector;
use std::ops::{Add, Neg};

mod clip;
mod note;
mod piano;
pub mod piano_roll;
pub mod playlist;
mod seeker;
mod track;

pub use clip::Clip;
pub use note::Note;
pub use piano::Piano;
pub use piano_roll::PianoRoll;
pub use playlist::Playlist;
pub use seeker::Seeker;
pub use track::Track;

pub const LINE_HEIGHT: f32 = TEXT_HEIGHT * 1.3;
pub const TEXT_HEIGHT: f32 = 16.0;

pub const ALPHA_1_3: f32 = 1.0 / 3.0;
pub const ALPHA_2_3: f32 = 2.0 / 3.0;

#[derive(Clone, Copy, Debug)]
pub enum Delta<T> {
	Positive(T),
	Negative(T),
}

impl<T> Delta<T> {
	pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Delta<U> {
		match self {
			Self::Positive(diff) => Delta::Positive(f(diff)),
			Self::Negative(diff) => Delta::Negative(f(diff)),
		}
	}
}

impl Add<Delta<Self>> for usize {
	type Output = Self;

	fn add(self, rhs: Delta<Self>) -> Self::Output {
		match rhs {
			Delta::Positive(diff) => self + diff,
			Delta::Negative(diff) => self.saturating_sub(diff),
		}
	}
}

impl Add<Delta<Self>> for BeatTime {
	type Output = Self;

	fn add(self, rhs: Delta<Self>) -> Self::Output {
		match rhs {
			Delta::Positive(diff) => self + diff,
			Delta::Negative(diff) => self.saturating_sub(diff),
		}
	}
}

impl Add<Delta<Self>> for SecondsTime {
	type Output = Self;

	fn add(self, rhs: Delta<Self>) -> Self::Output {
		match rhs {
			Delta::Positive(diff) => self + diff,
			Delta::Negative(diff) => self.saturating_sub(diff),
		}
	}
}

impl Add<Delta<Self>> for MidiKey {
	type Output = Self;

	fn add(self, rhs: Delta<Self>) -> Self::Output {
		match rhs {
			Delta::Positive(diff) => Self((self.0 + diff.0).min(127)),
			Delta::Negative(diff) => Self(self.0.saturating_sub(diff.0)),
		}
	}
}

impl<T> Neg for Delta<T> {
	type Output = Self;

	fn neg(self) -> Self::Output {
		match self {
			Self::Positive(diff) => Self::Negative(diff),
			Self::Negative(diff) => Self::Positive(diff),
		}
	}
}

pub fn frames_per_px(scale: Vector, transport: &Transport) -> f32 {
	(scale.x - 1.0).exp2() * transport.sample_rate.get() as f32
}

fn time_to_px(time: BeatTime, position: Vector, scale: Vector, transport: &Transport) -> f32 {
	(time.to_frames(transport) as f64 / f64::from(frames_per_px(scale, transport))
		- f64::from(position.x)) as f32
}

fn px_to_time(px: f32, position: Vector, scale: Vector, transport: &Transport) -> BeatTime {
	BeatTime::from_frames(
		((f64::from(px) + f64::from(position.x)) * f64::from(frames_per_px(scale, transport)))
			as usize,
		transport,
	)
}

fn key_to_px(key: MidiKey, position: Vector, scale: Vector) -> f32 {
	scale.y * f32::from(127 - key.0) - position.y
}

fn px_to_key(px: f32, position: Vector, scale: Vector) -> MidiKey {
	MidiKey(127 - ((px + position.y) / scale.y) as u8)
}
