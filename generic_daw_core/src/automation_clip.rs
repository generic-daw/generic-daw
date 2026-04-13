use crate::{AutomationPatternId, audio_thread::State, time::OffsetBeatRange};

#[derive(Clone, Copy, Debug)]
pub struct AutomationClip {
	pub pattern: AutomationPatternId,
	pub position: OffsetBeatRange,
}

impl AutomationClip {
	#[must_use]
	pub fn interpolate(&self, state: &State) -> f32 {
		let pattern = &state.automation_patterns[&self.pattern];

		let (start, end, offset) = self.position.to_samples(&state.transport);
		let now = state
			.transport
			.position
			.to_samples(&state.transport)
			.clamp(start, end)
			+ offset;

		pattern
			.points
			.array_windows()
			.map(|&[mut this, mut next]| {
				this.position += self.position.start();
				next.position += self.position.start();
				[this, next]
			})
			.find(|[_, next]| next.position.to_samples(&state.transport) > now)
			.map_or_else(
				|| pattern.points.last().unwrap().value,
				|[this, next]| this.interpolate(next, now, &state.transport),
			)
	}
}
