use crate::Transport;
use std::{
	fmt::{Debug, Formatter},
	ops::{Add, AddAssign, Sub, SubAssign},
	time::Duration,
};

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

	pub const TICKS_PER_BEAT: u64 = 28_224_000;

	#[must_use]
	pub const fn new(beat: u64, tick: u64) -> Self {
		debug_assert!(tick <= Self::TICKS_PER_BEAT);
		debug_assert!(beat <= u64::MAX / Self::TICKS_PER_BEAT);

		Self(beat * Self::TICKS_PER_BEAT + tick)
	}

	#[must_use]
	pub const fn from_raw(raw: u64) -> Self {
		Self(raw)
	}

	#[must_use]
	pub const fn into_raw(self) -> u64 {
		self.0
	}

	#[must_use]
	pub const fn bar(self, transport: &Transport) -> u64 {
		self.beat() / transport.numerator.get() as u64
	}

	#[must_use]
	pub const fn beat(self) -> u64 {
		self.0 / Self::TICKS_PER_BEAT
	}

	#[must_use]
	pub const fn beat_in_bar(self, transport: &Transport) -> u64 {
		self.beat() % transport.numerator.get() as u64
	}

	#[must_use]
	pub const fn tick(self) -> u64 {
		self.0 % Self::TICKS_PER_BEAT
	}

	#[must_use]
	pub const fn from_samples_f(samples: f32, transport: &Transport) -> Self {
		let samples = samples as f64;
		let bpm = transport.bpm.get() as f64;
		let sample_rate = transport.sample_rate.get() as f64;

		let time = samples * bpm * (Self::TICKS_PER_BEAT / 60 / 2) as f64 / sample_rate;

		Self(time as u64)
	}

	#[must_use]
	pub const fn from_samples(samples: usize, transport: &Transport) -> Self {
		debug_assert!(samples.is_multiple_of(2));

		let samples = samples as u64;
		let bpm = transport.bpm.get() as u64;
		let sample_rate = transport.sample_rate.get() as u64;

		let time = samples * bpm * (Self::TICKS_PER_BEAT / 60 / 2) / sample_rate;

		Self(time)
	}

	#[must_use]
	pub const fn to_samples_f(self, transport: &Transport) -> f32 {
		let beat = self.0 as f64;
		let bpm = transport.bpm.get() as f64;
		let sample_rate = transport.sample_rate.get() as f64;

		let samples = (beat * sample_rate) / bpm / (Self::TICKS_PER_BEAT / 60 / 2) as f64;

		samples as f32
	}

	#[must_use]
	pub const fn to_samples(self, transport: &Transport) -> usize {
		let time = self.0;
		let bpm = transport.bpm.get() as u64;
		let sample_rate = transport.sample_rate.get() as u64;

		let samples = (time * sample_rate) / bpm / (Self::TICKS_PER_BEAT / 60 / 2);

		samples.next_multiple_of(2) as usize
	}

	#[must_use]
	pub const fn from_duration(duration: Duration, transport: &Transport) -> Self {
		Self::from_samples_f(
			duration.as_secs_f32() * 2.0 * transport.sample_rate.get() as f32,
			transport,
		)
	}

	#[must_use]
	pub fn to_duration(self, transport: &Transport) -> Duration {
		Duration::from_secs_f32(
			self.to_samples_f(transport) / 2.0 / transport.sample_rate.get() as f32,
		)
	}

	#[must_use]
	pub const fn floor(mut self, modulo: Self) -> Self {
		self.0 -= self.0 % modulo.0;
		self
	}

	#[must_use]
	pub const fn ceil(mut self, modulo: Self) -> Self {
		self.0 += (modulo.0 - (self.0 % modulo.0)) % modulo.0;
		self
	}

	#[must_use]
	pub const fn round(mut self, modulo: Self) -> Self {
		let diff = self.0 % modulo.0;
		if diff < modulo.0 / 2 {
			self.0 -= diff;
		} else {
			self.0 += modulo.0 - diff;
		}
		self
	}

	#[must_use]
	pub const fn beat_floor(self) -> Self {
		self.floor(Self::BEAT)
	}

	#[must_use]
	pub const fn beat_ceil(self) -> Self {
		self.ceil(Self::BEAT)
	}

	#[must_use]
	pub const fn beat_round(self) -> Self {
		self.round(Self::BEAT)
	}

	#[must_use]
	pub const fn bar_floor(self, transport: &Transport) -> Self {
		self.floor(Self::new(transport.numerator.get() as u64, 0))
	}

	#[must_use]
	pub const fn bar_ceil(self, transport: &Transport) -> Self {
		self.ceil(Self::new(transport.numerator.get() as u64, 0))
	}

	#[must_use]
	pub const fn bar_round(self, transport: &Transport) -> Self {
		self.round(Self::new(transport.numerator.get() as u64, 0))
	}

	#[must_use]
	pub fn snap_floor(self, scale: f32, transport: &Transport) -> Self {
		self.floor(Self::snap_step(scale, transport))
	}

	#[must_use]
	pub fn snap_ceil(self, scale: f32, transport: &Transport) -> Self {
		self.ceil(Self::snap_step(scale, transport))
	}

	#[must_use]
	pub fn snap_round(self, scale: f32, transport: &Transport) -> Self {
		self.round(Self::snap_step(scale, transport))
	}

	#[must_use]
	pub fn snap_step(mut scale: f32, transport: &Transport) -> Self {
		scale += (f32::from(transport.bpm.get()) / transport.sample_rate.get() as f32).log2() - 3.5;
		let extra = f32::from(transport.numerator.get()).log2();
		if scale < 0.0 {
			Self::new(0, Self::TICKS_PER_BEAT >> -scale as u8)
		} else if scale < extra {
			Self::new(u64::from(transport.numerator.get()), 0)
		} else {
			Self::new(
				u64::from(transport.numerator.get()) << (scale - extra).ceil() as u8,
				0,
			)
		}
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
