use super::Host;
use crate::{host::HostThreadMessage, main_thread::MainThreadMessage};
use clack_host::{prelude::*, process::StartedPluginAudioProcessor};
use std::{
    fmt::Debug,
    sync::{
        atomic::{
            AtomicU64,
            Ordering::{Relaxed, SeqCst},
        },
        mpsc::{Receiver, Sender},
    },
};

pub struct PluginAudioProcessor {
    started_processor: Option<StartedPluginAudioProcessor<Host>>,
    pub steady_time: AtomicU64,
    pub sender: Sender<HostThreadMessage>,
    pub receiver: Receiver<MainThreadMessage>,
}

impl Debug for PluginAudioProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioProcessor")
            .field("steady_time", &self.steady_time.load(Relaxed))
            .finish_non_exhaustive()
    }
}

impl PluginAudioProcessor {
    pub(crate) fn new(
        audio_processor: StartedPluginAudioProcessor<Host>,
        sender: Sender<HostThreadMessage>,
        receiver: Receiver<MainThreadMessage>,
    ) -> Self {
        Self {
            started_processor: Some(audio_processor),
            steady_time: AtomicU64::new(0),
            sender,
            receiver,
        }
    }

    pub fn process(
        &mut self,
        input_audio_buffers: &mut [Vec<f32>],
        input_events_buffer: &EventBuffer,
        input_ports: &mut AudioPorts,
        output_ports: &mut AudioPorts,
    ) -> (Vec<Vec<f32>>, EventBuffer) {
        let mut output_audio_buffers = input_audio_buffers.to_owned();

        let input_audio = input_ports.with_input_buffers([AudioPortBuffer {
            latency: 0,
            channels: AudioPortBufferType::f32_input_only(
                input_audio_buffers.iter_mut().map(InputChannel::constant),
            ),
        }]);

        let mut output_audio = output_ports.with_output_buffers([AudioPortBuffer {
            latency: 0,
            channels: AudioPortBufferType::f32_output_only(
                output_audio_buffers.iter_mut().map(Vec::as_mut_slice),
            ),
        }]);

        let input_events = InputEvents::from_buffer(input_events_buffer);
        let mut output_events_buffer = EventBuffer::new();
        let mut output_events = OutputEvents::from_buffer(&mut output_events_buffer);

        self.started_processor
            .as_mut()
            .unwrap()
            .process(
                &input_audio,
                &mut output_audio,
                &input_events,
                &mut output_events,
                Some(self.steady_time.load(SeqCst)),
                None,
            )
            .unwrap();

        self.steady_time
            .fetch_add(u64::from(output_audio.frames_count().unwrap()), SeqCst);

        (output_audio_buffers, output_events_buffer)
    }
}
