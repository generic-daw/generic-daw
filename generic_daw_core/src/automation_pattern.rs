use crate::{AutomationPoint, MusicalTime};
use utils::unique_id;

unique_id!(automation_pattern_id);

pub use automation_pattern_id::Id as AutomationPatternId;

#[derive(Clone, Copy, Debug)]
pub enum AutomationPatternAction {
	Add(AutomationPoint, usize),
	Remove(usize),
	ChangeValue(usize, f32),
	MoveTo(usize, MusicalTime),
}

#[derive(Debug)]
pub struct AutomationPattern {
	pub id: AutomationPatternId,
	pub points: Vec<AutomationPoint>,
}

impl AutomationPattern {
	#[must_use]
	pub fn new(points: Vec<AutomationPoint>) -> Self {
		Self {
			id: AutomationPatternId::unique(),
			points,
		}
	}

	pub fn apply(&mut self, action: AutomationPatternAction) {
		match action {
			AutomationPatternAction::Add(point, index) => self.points.insert(index, point),
			AutomationPatternAction::Remove(index) => _ = self.points.remove(index),
			AutomationPatternAction::ChangeValue(index, value) => self.points[index].value = value,
			AutomationPatternAction::MoveTo(index, pos) => self.points[index].position = pos,
		}
	}
}
