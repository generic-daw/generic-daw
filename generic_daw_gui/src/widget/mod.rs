use iced::widget::text::Shaping;

mod arrangement;
mod arrangement_position;
mod arrangement_scale;
mod audio_clip;
mod bpm_input;
mod file_tree_entry;
mod file_tree_indicator;
mod knob;
mod peak_meter;
mod redrawer;
mod track;
mod vsplit;

pub use arrangement::Arrangement;
pub use arrangement_position::ArrangementPosition;
pub use arrangement_scale::ArrangementScale;
pub use audio_clip::AudioClip;
pub use bpm_input::BpmInput;
pub use file_tree_entry::FileTreeEntry;
pub use file_tree_indicator::FileTreeIndicator;
pub use knob::Knob;
pub use peak_meter::PeakMeter;
pub use redrawer::Redrawer;
pub use track::Track;
pub use vsplit::{Strategy, VSplit};

pub const LINE_HEIGHT: f32 = TEXT_HEIGHT * 1.3;
pub const TEXT_HEIGHT: f32 = 16.0;

pub const SWM: f32 = 60.0;

fn shaping_of(text: &str) -> Shaping {
    if text.is_ascii() {
        Shaping::Basic
    } else {
        Shaping::Advanced
    }
}
