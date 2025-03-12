use crate::{Host, PluginId, PluginType, audio_buffers::AudioBuffers, note_buffers::NoteBuffers};
use clack_host::process::StartedPluginAudioProcessor;
use generic_daw_utils::NoDebug;

#[derive(Debug)]
pub struct AudioProcessor {
    started_processor: NoDebug<StartedPluginAudioProcessor<Host>>,
    ty: PluginType,
    id: PluginId,
    steady_time: u64,
    audio_buffers: AudioBuffers,
    pub note_buffers: NoteBuffers,
}

impl AudioProcessor {
    #[must_use]
    pub fn new(
        started_processor: StartedPluginAudioProcessor<Host>,
        ty: PluginType,
        id: PluginId,
        audio_buffers: AudioBuffers,
        note_buffers: NoteBuffers,
    ) -> Self {
        Self {
            started_processor: started_processor.into(),
            ty,
            id,
            steady_time: 0,
            audio_buffers,
            note_buffers,
        }
    }

    #[must_use]
    pub fn id(&self) -> PluginId {
        self.id
    }

    pub fn process(&mut self, buf: &mut [f32], mix_level: f32) {
        if self.ty.note_output() {
            self.note_buffers.output_events.clear();
        }

        if self.ty.audio_input() {
            self.audio_buffers.read_in(buf);
        }

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

        if self.ty.audio_output() {
            self.audio_buffers.write_out(buf, mix_level);
        }

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
