use super::Host;
use clack_host::{prelude::*, process::StartedPluginAudioProcessor};
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};

pub struct AudioProcessor {
    started_audio_processor: Option<StartedPluginAudioProcessor<Host>>,
    steady_time: AtomicU64,
}

impl AudioProcessor {
    pub fn new(audio_processor: StartedPluginAudioProcessor<Host>) -> Self {
        Self {
            started_audio_processor: Some(audio_processor),
            steady_time: AtomicU64::new(0),
        }
    }

    pub fn steady_time(&self) -> u64 {
        self.steady_time.load(SeqCst)
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

        self.started_audio_processor
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
