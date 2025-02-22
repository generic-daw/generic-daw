use crate::{Meter, MidiClip, Position};
use audio_graph::{AudioGraphNodeImpl, MixerNode};
use clap_host::{HostAudioProcessor, PluginAudioProcessor, clack_host::prelude::EventBuffer};
use std::sync::Arc;

mod dirty_event;

pub use dirty_event::DirtyEvent;

#[derive(Clone, Debug)]
pub struct MidiTrack {
    host_audio_processor: Arc<HostAudioProcessor>,
    /// contains clips of midi patterns
    pub clips: Vec<Arc<MidiClip>>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
    /// volume and pan
    pub node: Arc<MixerNode>,
}

impl AudioGraphNodeImpl for MidiTrack {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        if let Ok((audio, _)) = self.host_audio_processor.receiver.try_recv() {
            buf.copy_from_slice(&audio[self.host_audio_processor.output_config.main_port_index]);
        }

        drop(
            self.host_audio_processor
                .sender
                .try_send(([].into(), EventBuffer::new())),
        );

        self.node.fill_buf(buf_start_sample, buf);
    }

    fn id(&self) -> audio_graph::NodeId {
        self.node.id()
    }
}

impl MidiTrack {
    #[must_use]
    pub fn new(
        meter: Arc<Meter>,
        node: Arc<MixerNode>,
        host_audio_processor: HostAudioProcessor,
        mut plugin_audio_processor: PluginAudioProcessor,
    ) -> Self {
        std::thread::spawn(move || {
            while let Ok((in_audio, in_events)) = plugin_audio_processor.receiver.recv_blocking() {
                assert!(in_audio.len() <= plugin_audio_processor.input_channels.len());

                plugin_audio_processor
                    .input_channels
                    .iter_mut()
                    .zip(&in_audio)
                    .for_each(|(buf, audio)| {
                        buf.copy_from_slice(audio);
                    });

                let mut output_events = EventBuffer::new();

                plugin_audio_processor
                    .process(&in_events.as_input(), &mut output_events.as_output());

                plugin_audio_processor
                    .sender
                    .send_blocking((
                        plugin_audio_processor.output_channels.clone(),
                        output_events,
                    ))
                    .unwrap();
            }
        });

        Self {
            host_audio_processor: Arc::new(host_audio_processor),
            clips: Vec::new(),
            meter,
            node,
        }
    }

    #[must_use]
    pub fn len(&self) -> Position {
        self.clips
            .iter()
            .map(|clip| clip.get_global_end())
            .max()
            .unwrap_or_default()
    }
}
