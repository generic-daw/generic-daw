use crate::{Host, audio_ports_config::AudioPortsConfig, buffers::Buffers};
use clack_host::{prelude::*, process::StartedPluginAudioProcessor};
use std::fmt::{Debug, Formatter};

pub struct AudioProcessor {
    started_processor: StartedPluginAudioProcessor<Host>,
    steady_time: u64,
    buffers: Buffers,
    pub input_events: EventBuffer,
    pub output_events: EventBuffer,
}

impl Debug for AudioProcessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginAudioProcessor")
            .field("steady_time", &self.steady_time)
            .field("buffers", &self.buffers)
            .field("input_events", &self.input_events)
            .field("output_events", &self.output_events)
            .finish_non_exhaustive()
    }
}

impl AudioProcessor {
    pub(crate) fn new(
        started_processor: StartedPluginAudioProcessor<Host>,
        config: PluginAudioConfiguration,
        input_config: AudioPortsConfig,
        output_config: AudioPortsConfig,
    ) -> Self {
        let buffers = Buffers::new(config, input_config, output_config);

        Self {
            started_processor,
            steady_time: 0,
            buffers,
            input_events: EventBuffer::with_capacity(config.max_frames_count as usize),
            output_events: EventBuffer::with_capacity(config.max_frames_count as usize),
        }
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        self.output_events.clear();

        self.buffers.read_in(buf);

        let (input_audio, mut output_audio) = self.buffers.prepare(buf);

        self.started_processor
            .process(
                &input_audio,
                &mut output_audio,
                &self.input_events.as_input(),
                &mut self.output_events.as_output(),
                Some(self.steady_time),
                None,
            )
            .unwrap();

        self.steady_time += u64::from(output_audio.frames_count().unwrap());

        self.buffers.write_out(buf);

        self.input_events.clear();
    }

    pub fn reset(&mut self) {
        self.started_processor.reset();
        self.steady_time = 0;
    }

    #[must_use]
    pub fn steady_time(&self) -> u64 {
        self.steady_time
    }
}
