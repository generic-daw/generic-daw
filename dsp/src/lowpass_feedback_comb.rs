use crate::DelayLine;

#[derive(Clone, Debug)]
pub struct LowpassFeedbackComb {
	delay_line: DelayLine,
	filter_state: f32,
	feedback: f32,
	dampening: (f32, f32),
}

impl LowpassFeedbackComb {
	#[must_use]
	pub fn new(len: usize) -> Self {
		Self {
			delay_line: DelayLine::new(len),
			filter_state: 0.0,
			feedback: 0.0,
			dampening: (0.0, 0.0),
		}
		.feedback(0.5)
		.dampening(0.5)
	}

	#[must_use]
	pub const fn feedback(mut self, feedback: f32) -> Self {
		self.feedback = feedback;
		self
	}

	#[must_use]
	pub const fn dampening(mut self, dampening: f32) -> Self {
		self.dampening = (dampening, 1.0 - dampening);
		self
	}

	#[must_use]
	pub fn tick(&mut self, input: f32) -> f32 {
		let delayed = self.delay_line.read();
		self.filter_state = self
			.filter_state
			.mul_add(self.dampening.0, delayed * self.dampening.1);
		self.delay_line
			.write(self.filter_state.mul_add(self.feedback, input))
	}

	pub fn process(&mut self, audio: &mut [f32]) {
		for sample in audio {
			*sample = self.tick(*sample);
		}
	}
}
