use generic_daw_core::{MidiKey, MusicalTime, Transport};
use iced::{Vector, keyboard::Modifiers};
use std::ops::Add;

pub mod clip;
pub mod piano;
pub mod piano_roll;
pub mod playlist;
pub mod seeker;
pub mod track;

pub const LINE_HEIGHT: f32 = TEXT_HEIGHT * 1.3;
pub const TEXT_HEIGHT: f32 = 16.0;

pub const OPACITY_33: f32 = 1.0 / 3.0;
pub const OPACITY_67: f32 = 2.0 / 3.0;

#[derive(Clone, Copy, Debug)]
pub enum Delta<T> {
	Positive(T),
	Negative(T),
}

impl Add<Delta<Self>> for MusicalTime {
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

fn maybe_snap<T>(t: T, modifiers: Modifiers, f: impl FnOnce(T) -> T) -> T {
	if modifiers.alt() { t } else { f(t) }
}

fn time_to_px(time: MusicalTime, position: Vector, scale: Vector, transport: &Transport) -> f32 {
	(time.to_samples(transport) as f64 / f64::from(scale.x.exp2()) - f64::from(position.x)) as f32
}

fn px_to_time(px: f32, position: Vector, scale: Vector, transport: &Transport) -> MusicalTime {
	MusicalTime::from_samples(
		((f64::from(px) + f64::from(position.x)) * f64::from(scale.x.exp2())) as usize,
		transport,
	)
}

fn key_to_px(key: MidiKey, position: Vector, scale: Vector) -> f32 {
	scale.y * f32::from(127 - key.0) - position.y
}

fn px_to_key(px: f32, position: Vector, scale: Vector) -> MidiKey {
	MidiKey(127 - ((px + position.y) / scale.y) as u8)
}
