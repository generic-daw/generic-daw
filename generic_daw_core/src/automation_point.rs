use crate::MusicalTime;

#[derive(Clone, Copy, Debug)]
pub enum AutomationTransition {
	Linear,
	UCos(f32),
	BCos(f32),
}

#[derive(Clone, Copy, Debug)]
pub struct AutomationPoint {
	pub value: f32,
	pub position: MusicalTime,
	pub to_next: AutomationTransition,
}
