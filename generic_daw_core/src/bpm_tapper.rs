use std::{
	num::NonZero,
	time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default)]
pub struct BpmTapper {
	interval: Option<Duration>,
	tap: Option<Instant>,
}

impl BpmTapper {
	pub fn tap(&mut self) {
		let now = Instant::now();

		let Some(then) = self.tap.replace(now) else {
			return;
		};

		let diff = now - then;

		self.interval = if diff > Duration::from_secs(2) {
			None
		} else if let Some(interval) = self.interval
			&& (interval / 2..=interval * 2).contains(&diff)
		{
			Some((3 * interval + diff) / 4)
		} else {
			Some(diff)
		};
	}

	#[must_use]
	pub fn get_bpm(&self) -> Option<NonZero<u16>> {
		NonZero::new((60.0 / self.interval?.as_secs_f32()).round() as u16)
	}
}
