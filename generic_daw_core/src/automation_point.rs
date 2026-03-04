use crate::{MusicalTime, Transport};
use std::f32::consts::{FRAC_PI_2, PI};

#[derive(Clone, Copy, Debug)]
pub enum AutomationTransition {
	Linear,
	UCos(f32),
	BCos(f32),
}

impl AutomationTransition {
	#[must_use]
	pub fn interpolate(self, from: f32, to: f32, amt: f32) -> f32 {
		let linear = from * (1.0 - amt) + to * amt;
		match self {
			Self::Linear => linear,
			Self::UCos(mix) => {
				let amt = (FRAC_PI_2 * (amt - 1.0)).cos();
				let ucos = from * (1.0 - amt) + to * amt;
				mix * (linear - ucos) + linear
			}
			Self::BCos(mix) => {
				let amt = 0.5 * ((PI * (amt + 1.0)).cos() + 1.0);
				let bcos = from * (1.0 - amt) + to * amt;
				mix * (linear - bcos) + linear
			}
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct AutomationPoint {
	pub value: f32,
	pub position: MusicalTime,
	pub to_next: AutomationTransition,
}

impl AutomationPoint {
	#[must_use]
	pub fn interpolate(self, next: Self, time: usize, transport: &Transport) -> f32 {
		let self_time = self.position.to_samples(transport);
		let next_time = next.position.to_samples(transport);

		let amt = (time - self_time) as f32 / (next_time - self_time) as f32;

		self.to_next
			.interpolate(self.value, next.value, amt.clamp(0.0, 1.0))
	}
}
