use generic_daw_core::{MusicalTime, RtState};
use generic_daw_utils::Vec2;
use iced::keyboard::Modifiers;

pub mod arrangement;
pub mod clip;
pub mod piano;
pub mod piano_roll;
pub mod seeker;
pub mod track;

pub const LINE_HEIGHT: f32 = TEXT_HEIGHT * 1.3;
pub const TEXT_HEIGHT: f32 = 16.0;

fn get_unsnapped_time(x: f32, position: Vec2, scale: Vec2, rtstate: &RtState) -> MusicalTime {
	MusicalTime::from_samples_f(x.mul_add(scale.x.exp2(), position.x).max(0.0), rtstate)
}

fn get_time(
	x: f32,
	position: Vec2,
	scale: Vec2,
	rtstate: &RtState,
	modifiers: Modifiers,
) -> MusicalTime {
	let mut time = get_unsnapped_time(x, position, scale, rtstate);
	if !modifiers.alt() {
		time = time.snap_round(scale.x, rtstate);
	}
	time
}
