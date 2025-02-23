use crate::{Host, audio_ports_config::AudioPortsConfig, buffers::Buffers};
use clack_host::{prelude::*, process::StartedPluginAudioProcessor};
use std::fmt::{Debug, Formatter};

pub struct AudioProcessor {
    started_processor: StartedPluginAudioProcessor<Host>,
    steady_time: u64,
    buffers: Buffers,
}

impl Debug for AudioProcessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginAudioProcessor")
            .field("steady_time", &self.steady_time)
            .field("buffers", &self.buffers)
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
        }
    }

    pub fn process(
        &mut self,
        buf: &mut [f32],
        input_events: &InputEvents<'_>,
        output_events: &mut OutputEvents<'_>,
    ) {
        self.buffers.read_in(buf);

        let (input_audio, mut output_audio) = self.buffers.prepare(buf);

        self.started_processor
            .process(
                &input_audio,
                &mut output_audio,
                input_events,
                output_events,
                Some(self.steady_time),
                None,
            )
            .unwrap();

        self.steady_time += u64::from(output_audio.frames_count().unwrap());

        self.buffers.write_out(buf);
    }

    pub fn reset(&mut self) {
        self.started_processor.reset();
        self.steady_time = 0;
    }
}
