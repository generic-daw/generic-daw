use generic_daw_core::{MusicalTime, RtState};
use generic_daw_utils::Vec2;
use iced::keyboard::Modifiers;

pub mod arrangement;
pub mod audio_clip;
pub mod midi_clip;
pub mod piano;
pub mod piano_roll;
pub mod seeker;
pub mod track;
pub mod waveform;

pub const LINE_HEIGHT: f32 = TEXT_HEIGHT * 1.3;
pub const TEXT_HEIGHT: f32 = 16.0;

fn get_time(
	x: f32,
	modifiers: Modifiers,
	rtstate: &RtState,
	position: Vec2,
	scale: Vec2,
) -> MusicalTime {
	let time = x.mul_add(scale.x.exp2(), position.x).max(0.0);
	let mut time = MusicalTime::from_samples_f(time, rtstate);

	if !modifiers.alt() {
		time = time.snap_round(scale.x, rtstate);
	}

	time
}
