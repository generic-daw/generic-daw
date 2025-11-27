use crate::RtState;
use std::{
	fmt::{Debug, Formatter},
	ops::{Add, AddAssign, Mul, Sub, SubAssign},
};

mod clip_position;
mod note_position;

pub use clip_position::ClipPosition;
pub use note_position::NotePosition;

#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MusicalTime(u64);

impl Debug for MusicalTime {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("MusicalTime")
			.field("beat", &self.beat())
			.field("tick", &self.tick())
			.finish()
	}
}

impl MusicalTime {
	pub const ZERO: Self = Self::new(0, 0);
	pub const BEAT: Self = Self::new(1, 0);
	pub const TICK: Self = Self::new(0, 1);

	const TICK_BITS: u8 = 16;
	pub const TICKS_PER_BEAT: u64 = 1 << Self::TICK_BITS;

	#[must_use]
	pub const fn new(beat: u64, tick: u64) -> Self {
		debug_assert!(tick <= Self::TICKS_PER_BEAT);
		debug_assert!(beat <= u64::MAX / Self::TICKS_PER_BEAT);

		Self(beat * Self::TICKS_PER_BEAT + tick)
	}

	#[must_use]
	pub const fn bar(self, rtstate: &RtState) -> u64 {
		self.beat() / rtstate.numerator.get() as u64
	}

	#[must_use]
	pub const fn beat(self) -> u64 {
		self.0 / Self::TICKS_PER_BEAT
	}

	#[must_use]
	pub const fn beat_in_bar(self, rtstate: &RtState) -> u64 {
		self.beat() % rtstate.numerator.get() as u64
	}

	#[must_use]
	pub const fn tick(self) -> u64 {
		self.0 % Self::TICKS_PER_BEAT
	}

	#[must_use]
	pub const fn floor(mut self) -> Self {
		self.0 -= self.0 % Self::TICKS_PER_BEAT;
		self
	}

	#[must_use]
	pub const fn ceil(mut self) -> Self {
		self.0 += Self::TICKS_PER_BEAT - (self.0 % Self::TICKS_PER_BEAT);
		self
	}

	#[must_use]
	pub const fn round(mut self) -> Self {
		let diff = self.0 % Self::TICKS_PER_BEAT;
		if diff < Self::TICKS_PER_BEAT / 2 {
			self.0 -= diff;
		} else {
			self.0 += Self::TICKS_PER_BEAT - diff;
		}
		self
	}

	#[must_use]
	pub const fn from_samples_f(samples: f32, rtstate: &RtState) -> Self {
		let samples = samples as f64;
		let bpm = rtstate.bpm.get() as f64;
		let sample_rate = rtstate.sample_rate.get() as f64;

		let time = (samples * bpm * (Self::TICKS_PER_BEAT / 2) as f64) / (sample_rate * 60.0);

		Self(time as u64)
	}

	#[must_use]
	pub const fn from_samples(samples: usize, rtstate: &RtState) -> Self {
		debug_assert!(samples.is_multiple_of(2));

		let samples = samples as u64;
		let bpm = rtstate.bpm.get() as u64;
		let sample_rate = rtstate.sample_rate.get() as u64;

		let time = (samples * bpm * (Self::TICKS_PER_BEAT / 2)) / (sample_rate * 60);

		Self(time)
	}

	#[must_use]
	pub const fn to_samples_f(self, rtstate: &RtState) -> f32 {
		let beat = self.0 as f64;
		let bpm = rtstate.bpm.get() as f64;
		let sample_rate = rtstate.sample_rate.get() as f64;

		let samples = (beat * sample_rate * 60.0) / (bpm * (Self::TICKS_PER_BEAT / 2) as f64);

		samples as f32
	}

	#[must_use]
	pub const fn to_samples(self, rtstate: &RtState) -> usize {
		let time = self.0;
		let bpm = rtstate.bpm.get() as u64;
		let sample_rate = rtstate.sample_rate.get() as u64;

		let samples = (time * sample_rate * 60) / (bpm * (Self::TICKS_PER_BEAT / 2));

		samples.next_multiple_of(2) as usize
	}

	#[must_use]
	pub fn snap_floor(mut self, scale: f32, rtstate: &RtState) -> Self {
		let snap_step = Self::snap_step(scale, rtstate).0;
		self.0 -= self.0 % snap_step;
		self
	}

	#[must_use]
	pub fn snap_ceil(mut self, scale: f32, rtstate: &RtState) -> Self {
		let snap_step = Self::snap_step(scale, rtstate).0;
		self.0 += snap_step - (self.0 % snap_step);
		self
	}

	#[must_use]
	pub fn snap_round(mut self, scale: f32, rtstate: &RtState) -> Self {
		let modulo = Self::snap_step(scale, rtstate).0;
		let diff = self.0 % modulo;
		if diff < modulo / 2 {
			self.0 -= diff;
		} else {
			self.0 += modulo - diff;
		}
		self
	}

	#[must_use]
	pub fn snap_step(mut scale: f32, rtstate: &RtState) -> Self {
		scale += (f32::from(rtstate.bpm.get()) / rtstate.sample_rate.get() as f32).log2() - 3.0
			+ f32::from(Self::TICK_BITS);
		let extra = f32::from(rtstate.numerator.get()).log2();
		Self(if scale < f32::from(Self::TICK_BITS + 1) {
			1 << scale as u8
		} else if scale < f32::from(Self::TICK_BITS + 1) + extra {
			u64::from(rtstate.numerator.get()) << Self::TICK_BITS
		} else {
			u64::from(rtstate.numerator.get()) << (scale - extra) as u8
		})
	}

	#[must_use]
	pub const fn saturating_sub(self, other: Self) -> Self {
		Self(self.0.saturating_sub(other.0))
	}

	#[must_use]
	pub const fn abs_diff(self, other: Self) -> Self {
		Self(self.0.abs_diff(other.0))
	}
}

impl Add for MusicalTime {
	type Output = Self;

	fn add(self, rhs: Self) -> Self::Output {
		Self(self.0 + rhs.0)
	}
}

impl AddAssign for MusicalTime {
	fn add_assign(&mut self, rhs: Self) {
		self.0 += rhs.0;
	}
}

impl Sub for MusicalTime {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self::Output {
		Self(self.0 - rhs.0)
	}
}

impl SubAssign for MusicalTime {
	fn sub_assign(&mut self, rhs: Self) {
		self.0 -= rhs.0;
	}
}

impl Mul<u64> for MusicalTime {
	type Output = Self;

	fn mul(mut self, rhs: u64) -> Self::Output {
		self.0 *= rhs;
		self
	}
}

impl From<u64> for MusicalTime {
	fn from(value: u64) -> Self {
		Self(value)
	}
}

impl From<MusicalTime> for u64 {
	fn from(value: MusicalTime) -> Self {
		value.0
	}
}
