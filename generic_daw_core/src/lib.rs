use cpal::traits::{DeviceTrait as _, HostTrait as _};
use std::sync::Arc;

mod audio_clip;
mod audio_graph_node;
mod automation_clip;
mod automation_lane;
mod automation_pattern;
mod automation_point;
mod channel;
mod clip;
mod daw_ctx;
mod event;
mod export;
mod midi_clip;
mod midi_note;
mod midi_pattern;
mod musical_time;
mod offset_position;
mod position;
mod recording;
mod resampler;
mod sample;
mod stream;
mod track;

pub use audio_clip::AudioClip;
pub use audio_graph::{NodeId, NodeImpl};
pub use audio_graph_node::AudioGraphNode;
pub use automation_clip::AutomationClip;
pub use automation_lane::AutomationLane;
pub use automation_pattern::{AutomationPattern, AutomationPatternAction, AutomationPatternId};
pub use automation_point::{AutomationPoint, AutomationTransition};
pub use channel::{Channel, PanMode, PluginId};
pub use clap_host;
pub use clip::Clip;
pub use cpal::{Stream, traits::StreamTrait};
pub use daw_ctx::{Batch, Message, NodeAction, Transport, Update, Version};
pub use event::Event;
pub use export::Export;
pub use midi_clip::MidiClip;
pub use midi_note::{Key, MidiKey, MidiNote};
pub use midi_pattern::{MidiPattern, MidiPatternAction, MidiPatternId};
pub use musical_time::MusicalTime;
pub use offset_position::OffsetPosition;
pub use position::Position;
pub use recording::Recording;
pub use sample::{Sample, SampleId};
pub use stream::{build_input_stream, build_output_stream};
pub use symphonia::core::io::MediaSource;
pub use track::Track;

pub type AudioGraph = audio_graph::AudioGraph<AudioGraphNode>;

#[must_use]
pub fn input_devices() -> Vec<Arc<str>> {
	cpal::default_host()
		.input_devices()
		.unwrap()
		.filter_map(|device| device.description().ok())
		.map(|description| description.name().into())
		.collect()
}

#[must_use]
pub fn output_devices() -> Vec<Arc<str>> {
	cpal::default_host()
		.output_devices()
		.unwrap()
		.filter_map(|device| device.description().ok())
		.map(|description| description.name().into())
		.collect()
}
