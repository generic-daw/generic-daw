use generic_daw_core::{Meter, Position};
use generic_daw_utils::Vec2;
use iced::{keyboard::Modifiers, widget::text::Shaping};
use std::sync::atomic::Ordering::Acquire;

mod arrangement;
mod audio_clip;
mod bpm_input;
mod clipped;
mod knob;
mod midi_clip;
mod peak_meter;
mod piano;
mod piano_roll;
mod redrawer;
mod seeker;
mod track;
mod vsplit;

pub use arrangement::Arrangement;
pub use audio_clip::AudioClip;
pub use bpm_input::BpmInput;
pub use clipped::Clipped;
pub use knob::Knob;
pub use midi_clip::MidiClip;
pub use peak_meter::PeakMeter;
pub use piano::Piano;
pub use piano_roll::PianoRoll;
pub use redrawer::Redrawer;
pub use seeker::Seeker;
pub use track::Track;
pub use vsplit::{Strategy, VSplit};

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

fn get_time(x: f32, modifiers: Modifiers, meter: &Meter, position: Vec2, scale: Vec2) -> Position {
    let time = x.mul_add(scale.x.exp2(), position.x).max(0.0);
    let mut time =
        Position::from_interleaved_samples_f(time, meter.bpm.load(Acquire), meter.sample_rate);

    if !modifiers.alt() {
        time = time.snap(scale.x, meter.numerator.load(Acquire));
    }

    time
}
