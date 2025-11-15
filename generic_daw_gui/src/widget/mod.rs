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

fn get_unsnapped_time(x: f32, position: Vector, scale: Vector, rtstate: &RtState) -> MusicalTime {
	MusicalTime::from_samples_f(x.mul_add(scale.x.exp2(), position.x).max(0.0), rtstate)
}

fn get_time(
	x: f32,
	position: Vector,
	scale: Vector,
	rtstate: &RtState,
	modifiers: Modifiers,
) -> MusicalTime {
	let mut time = get_unsnapped_time(x, position, scale, rtstate);
	if !modifiers.alt() {
		time = time.snap_round(scale.x, rtstate);
	}
	time
}
