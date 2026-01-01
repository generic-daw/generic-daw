use crate::{AutomationPatternId, AutomationTransition, OffsetPosition, daw_ctx::State};
use std::f32::consts::{FRAC_PI_2, PI};

#[derive(Clone, Copy, Debug)]
pub struct AutomationClip {
	pub pattern: AutomationPatternId,
	pub position: OffsetPosition,
}

impl AutomationClip {
	#[must_use]
	pub fn interpolate(&self, state: &State) -> f32 {
		let pattern = &state.automation_patterns[*self.pattern];

		let start = self.position.start().to_samples(&state.transport);
		let now = state.transport.sample - start;

		pattern
			.points
			.windows(2)
			.map(|points| {
				let &[mut this, mut next] = points else {
					unreachable!()
				};
				this.position += self.position.offset();
				next.position += self.position.offset();
				[this, next]
			})
			.find(|[_, next]| next.position.to_samples(&state.transport) > now)
			.map_or_else(
				|| pattern.points.last().unwrap().value,
				|[this, next]| {
					let this_time = this.position.to_samples(&state.transport);
					let next_time = next.position.to_samples(&state.transport);

					if now < this_time {
						return this.value;
					}

					if this_time == next_time {
						return next.value;
					}

					let amt = (now - this_time) as f32 / (next_time - this_time) as f32;
					let linear = amt.mul_add(next.value, this.value * (1.0 - amt));

					match this.to_next {
						AutomationTransition::Linear => linear,
						AutomationTransition::UCos(mix) => {
							let amt = amt.mul_add(FRAC_PI_2, -FRAC_PI_2).cos();
							let ucos = amt.mul_add(next.value, this.value * (1.0 - amt));
							mix.mul_add(linear - ucos, linear)
						}
						AutomationTransition::BCos(mix) => {
							let amt = amt.mul_add(PI, PI).cos().mul_add(0.5, 0.5);
							let bcos = amt.mul_add(next.value, this.value * (1.0 - amt));
							mix.mul_add(linear - bcos, linear)
						}
					}
				},
			)
	}
}
