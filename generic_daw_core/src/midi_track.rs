use crate::{Meter, MidiClip, Position};
use audio_graph::{AudioGraphNodeImpl, MixerNode};
use clap_host::{
    HostAudioProcessor, PluginAudioProcessor,
    clack_host::prelude::{AudioPorts, EventBuffer},
};
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
        while let Ok((audio, _)) = self.host_audio_processor.receiver.try_recv() {
            audio[0]
                .iter()
                .zip(&audio[1])
                .flat_map(<[&f32; 2]>::from)
                .zip(&mut *buf)
                .for_each(|(sample, buf)| {
                    *buf = *sample;
                });
        }

        self.host_audio_processor
            .sender
            .send_blocking((vec![vec![]; 2], EventBuffer::new()))
            .unwrap();

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
            while let Ok((mut in_audio, in_events)) =
                plugin_audio_processor.receiver.recv_blocking()
            {
                let (out_audio, out_events) = plugin_audio_processor.process(
                    &mut in_audio,
                    &in_events,
                    &mut AudioPorts::with_capacity(0, 0),
                    &mut AudioPorts::with_capacity(2, 2),
                );
                plugin_audio_processor
                    .sender
                    .send_blocking((out_audio, out_events))
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
