use crate::{Host, audio_buffers::AudioBuffers, note_buffers::NoteBuffers};
use clack_host::process::StartedPluginAudioProcessor;
use std::fmt::{Debug, Formatter};

pub struct AudioProcessor {
    started_processor: StartedPluginAudioProcessor<Host>,
    steady_time: u64,
    audio_buffers: AudioBuffers,
    pub note_buffers: NoteBuffers,
}

impl Debug for AudioProcessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginAudioProcessor")
            .field("steady_time", &self.steady_time)
            .field("audio_buffers", &self.audio_buffers)
            .field("note_buffers", &self.note_buffers)
            .finish_non_exhaustive()
    }
}

impl AudioProcessor {
    #[must_use]
    pub fn new(
        started_processor: StartedPluginAudioProcessor<Host>,
        audio_buffers: AudioBuffers,
        note_buffers: NoteBuffers,
    ) -> Self {
        Self {
            started_processor,
            steady_time: 0,
            audio_buffers,
            note_buffers,
        }
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        self.note_buffers.output_events.clear();

        self.audio_buffers.read_in(buf);

        let (input_audio, mut output_audio) = self.audio_buffers.prepare(buf);

        self.started_processor
            .process(
                &input_audio,
                &mut output_audio,
                &self.note_buffers.input_events.as_input(),
                &mut self.note_buffers.output_events.as_output(),
                Some(self.steady_time),
                None,
            )
            .unwrap();

        self.steady_time += u64::from(output_audio.frames_count().unwrap());

        self.audio_buffers.write_out(buf);

        self.note_buffers.input_events.clear();
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
