use generic_daw_core::{MidiKey, MusicalTime, RtState};
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

fn get_time(x: f32, position: Vector, scale: Vector, rtstate: &RtState) -> MusicalTime {
	MusicalTime::from_samples_f(((x + position.x) * scale.x.exp2()).max(0.0), rtstate)
}

fn maybe_snap_time(
	time: MusicalTime,
	modifiers: Modifiers,
	f: impl FnOnce(MusicalTime) -> MusicalTime,
) -> MusicalTime {
	if modifiers.alt() { time } else { f(time) }
}

fn key_y(key: MidiKey, position: Vector, scale: Vector) -> f32 {
	scale.y.mul_add(127.0 - f32::from(key.0), -position.y)
}
