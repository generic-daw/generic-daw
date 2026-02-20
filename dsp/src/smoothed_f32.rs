#[derive(Clone, Copy, Debug, Default)]
pub struct SmoothedF32 {
	mem: f32,
	goal: f32,
	fac: f32,
}

impl SmoothedF32 {
	#[must_use]
	pub fn new(sample_rate: f32, value: f32) -> Self {
		Self {
			mem: value,
			goal: value,
			fac: 100.0 / sample_rate,
		}
	}

	pub fn set(&mut self, goal: f32) {
		self.goal = goal;
	}

	#[must_use]
	pub fn tick(&mut self) -> f32 {
		self.mem = self.mem.mul_add(1.0 - self.fac, self.goal * self.fac);
		self.mem
	}

	pub fn settle(&mut self) {
		if (self.mem - self.goal).abs() < self.goal.abs().mul_add(f32::EPSILON, f32::EPSILON) {
			self.mem = self.goal;
		}
	}
}
