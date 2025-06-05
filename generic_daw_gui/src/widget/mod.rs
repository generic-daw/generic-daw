use generic_daw_core::{MusicalTime, RtState};
use generic_daw_utils::Vec2;
use iced::{keyboard::Modifiers, widget::text::Shaping};

mod animated_dot;
pub mod arrangement;
mod audio_clip;
pub mod drag_handle;
mod knob;
mod midi_clip;
mod peak_meter;
mod piano;
pub mod piano_roll;
mod recording;
mod seeker;
mod track;
mod waveform;

pub use animated_dot::AnimatedDot;
pub use arrangement::Arrangement;
pub use audio_clip::AudioClip;
pub use drag_handle::DragHandle;
pub use knob::Knob;
pub use midi_clip::MidiClip;
pub use peak_meter::PeakMeter;
pub use piano::Piano;
pub use piano_roll::PianoRoll;
pub use recording::Recording;
pub use seeker::Seeker;
pub use track::Track;

pub const LINE_HEIGHT: f32 = TEXT_HEIGHT * 1.3;
pub const TEXT_HEIGHT: f32 = 16.0;

pub const SWM: f32 = 60.0;

pub fn shaping_of(text: &str) -> Shaping {
    if text.is_ascii() {
        Shaping::Basic
    } else {
        Shaping::Advanced
    }
}

#[expect(clippy::trivially_copy_pass_by_ref)]
fn get_time(
    x: f32,
    modifiers: Modifiers,
    rtstate: &RtState,
    position: &Vec2,
    scale: &Vec2,
) -> MusicalTime {
    let time = x.mul_add(scale.x.exp2(), position.x).max(0.0);
    let mut time = MusicalTime::from_samples_f(time, rtstate);

    if !modifiers.alt() {
        time = time.snap_round(scale.x, rtstate);
    }

    time
}
