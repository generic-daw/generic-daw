use crate::{
    Host, PluginDescriptor, PluginId, audio_buffers::AudioBuffers, note_buffers::NoteBuffers,
};
use async_channel::Receiver;
use clack_host::process::StartedPluginAudioProcessor;
use generic_daw_utils::NoDebug;
use tracing::info;

#[derive(Clone, Copy, Debug)]
pub enum AudioThreadMessage {
    RequestRestart,
    LatencyChanged(u32),
}

#[derive(Debug)]
pub struct AudioProcessor {
    started_processor: Option<NoDebug<StartedPluginAudioProcessor<Host>>>,
    descriptor: PluginDescriptor,
    id: PluginId,
    steady_time: u64,
    audio_buffers: AudioBuffers,
    pub note_buffers: NoteBuffers,
    receiver: Receiver<AudioThreadMessage>,
}

impl AudioProcessor {
    #[must_use]
    pub fn new(
        started_processor: StartedPluginAudioProcessor<Host>,
        descriptor: PluginDescriptor,
        id: PluginId,
        audio_buffers: AudioBuffers,
        note_buffers: NoteBuffers,
        receiver: Receiver<AudioThreadMessage>,
    ) -> Self {
        Self {
            started_processor: Some(started_processor.into()),
            descriptor,
            id,
            steady_time: 0,
            audio_buffers,
            note_buffers,
            receiver,
        }
    }

    #[must_use]
    pub fn id(&self) -> PluginId {
        self.id
    }

    pub fn process(&mut self, buf: &mut [f32], mix_level: f32) {
        while let Ok(msg) = self.receiver.try_recv() {
            match msg {
                AudioThreadMessage::RequestRestart => {
                    info!(
                        "{} ({}): restarting audio processor",
                        self.descriptor.name, self.descriptor.id
                    );

                    let mut stopped_processor =
                        self.started_processor.take().unwrap().0.stop_processing();

                    let started_processor = loop {
                        match stopped_processor.start_processing() {
                            Ok(started_processor) => break started_processor,
                            Err(err) => stopped_processor = err.into_stopped_processor(),
                        }
                    };

                    self.started_processor = Some(started_processor.into());
                }
                AudioThreadMessage::LatencyChanged(latency) => {
                    info!(
                        "{} ({}): setting latency to {latency} frames",
                        self.descriptor.name, self.descriptor.id
                    );

                    self.audio_buffers.latency_changed(latency);
                }
            }
        }

        if self.descriptor.ty.note_output() {
            self.note_buffers.output_events.clear();
        }

        if self.descriptor.ty.audio_input() {
            self.audio_buffers.read_in(buf);
        }

        let (input_audio, mut output_audio) = self.audio_buffers.prepare(buf);

        self.started_processor
            .as_mut()
            .unwrap()
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

        if self.descriptor.ty.audio_output() {
            self.audio_buffers.write_out(buf, mix_level);
        }

        self.note_buffers.input_events.clear();
    }

    pub fn reset(&mut self) {
        self.started_processor.as_mut().unwrap().reset();
        self.steady_time = 0;
    }

    #[must_use]
    pub fn steady_time(&self) -> u64 {
        self.steady_time
    }
}
