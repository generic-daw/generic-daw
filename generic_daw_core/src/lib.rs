use cpal::traits::{DeviceTrait as _, HostTrait as _};
use std::sync::Arc;

mod audio_clip;
mod audio_graph_node;
mod automation_clip;
mod automation_lane;
mod automation_pattern;
mod channel;
mod clip;
mod daw_ctx;
mod event;
mod export;
mod midi_clip;
mod midi_note;
mod midi_pattern;
mod musical_time;
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
pub use automation_pattern::{
	AutomationPattern, AutomationPatternId, AutomationPoint, AutomationTransition,
};
pub use channel::{Channel, PanMode};
pub use clap_host;
pub use clip::Clip;
pub use daw_ctx::{Batch, Message, NodeAction, Transport, Update, Version};
pub use event::Event;
pub use export::Export;
pub use midi_clip::MidiClip;
pub use midi_note::{Key, MidiKey, MidiNote};
pub use midi_pattern::{MidiPattern, MidiPatternAction, MidiPatternId};
pub use musical_time::{ClipPosition, MusicalTime, NotePosition};
pub use recording::Recording;
pub use sample::{Sample, SampleId};
pub use stream::{
	InputRequest, InputResponse, OutputRequest, OutputResponse, STREAM_THREAD, StreamMessage,
	StreamToken,
};
pub use symphonia::core::io::MediaSource;
pub use track::Track;

pub type AudioGraph = audio_graph::AudioGraph<AudioGraphNode>;

#[must_use]
pub fn get_input_devices() -> Vec<Arc<str>> {
	cpal::default_host()
		.input_devices()
		.unwrap()
		.filter_map(|device| device.name().ok())
		.map(Arc::from)
		.collect()
}

#[must_use]
pub fn get_output_devices() -> Vec<Arc<str>> {
	cpal::default_host()
		.output_devices()
		.unwrap()
		.filter_map(|device| device.name().ok())
		.map(Arc::from)
		.collect()
}
