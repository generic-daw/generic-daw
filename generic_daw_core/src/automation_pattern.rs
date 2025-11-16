use crate::MusicalTime;
use generic_daw_utils::unique_id;

unique_id!(automation_pattern_id);

pub use automation_pattern_id::Id as AutomationPatternId;

#[derive(Clone, Copy, Debug, Default)]
pub enum AutomationTransition {
	#[default]
	Linear,
	UCos(f32),
	BCos(f32),
}

#[derive(Clone, Copy, Debug)]
pub struct AutomationPoint {
	pub time: MusicalTime,
	pub value: f32,
	pub to_next: AutomationTransition,
}

#[derive(Debug)]
pub struct AutomationPattern {
	pub id: AutomationPatternId,
	pub points: Vec<AutomationPoint>,
}
