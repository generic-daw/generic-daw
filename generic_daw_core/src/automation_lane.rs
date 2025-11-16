use crate::{AutomationClip, Event, daw_ctx::State};
use clap_host::{ClapId, Cookie};

#[derive(Debug)]
pub struct AutomationLane {
	clips: Vec<AutomationClip>,
	param_id: ClapId,
	cookie: Cookie,
	last_value: f32,
}

impl AutomationLane {
	pub fn process(&mut self, state: &State, events: &mut Vec<Event>) {
		let mut inside = None::<&AutomationClip>;
		let mut after = None::<&AutomationClip>;
		let mut before = None::<&AutomationClip>;

		for clip in &self.clips {
			let start = clip.position.start().to_samples(&state.rtstate);
			let end = clip.position.end().to_samples(&state.rtstate);

			match (start <= state.rtstate.sample, end >= state.rtstate.sample) {
				(true, true) => {
					inside = inside
						.filter(|inside| inside.position.start() > clip.position.start())
						.or(Some(clip));
				}
				(true, false) => {
					before = before
						.filter(|before| before.position.end() > clip.position.end())
						.or(Some(clip));
				}
				(false, true) => {
					after = after
						.filter(|after| after.position.start() < clip.position.start())
						.or(Some(clip));
				}
				(false, false) => unreachable!(),
			}
		}

		if let Some(clip) = inside.or(before).or(after) {
			let value = clip.interpolate(state);

			if value != self.last_value {
				self.last_value = value;
				events.push(Event::ParamValue {
					time: 0,
					param_id: self.param_id,
					value,
					cookie: self.cookie,
				});
			}
		}
	}
}
