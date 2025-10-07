use cpal::{
	BufferSize, StreamConfig,
	traits::{DeviceTrait as _, HostTrait as _},
};
use master::Master;
use std::sync::Arc;

mod audio_clip;
mod audio_graph_node;
mod channel;
mod clip;
mod daw_ctx;
mod event;
mod export;
mod master;
mod midi_clip;
mod midi_note;
mod musical_time;
mod pattern;
mod recording;
mod resampler;
mod sample;
mod stream;
mod track;

pub use audio_clip::AudioClip;
pub use audio_graph::{NodeId, NodeImpl};
pub use audio_graph_node::AudioGraphNode;
pub use channel::{Channel, Flags, PanMode};
pub use clap_host;
pub use clip::Clip;
pub use daw_ctx::{Batch, Message, NodeAction, PatternAction, RtState, Update, Version};
pub use event::Event;
pub use export::Export;
pub use midi_clip::MidiClip;
pub use midi_note::{Key, MidiKey, MidiNote};
pub use musical_time::{ClipPosition, MusicalTime, NotePosition};
pub use pattern::{Pattern, PatternId};
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

fn buffer_size_of_config(config: &StreamConfig) -> Option<u32> {
	match config.buffer_size {
		BufferSize::Fixed(buffer_size) => Some(buffer_size),
		BufferSize::Default => None,
	}
}
