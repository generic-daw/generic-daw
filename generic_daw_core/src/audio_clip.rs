use crate::{Meter, Position, clip_position::ClipPosition};
use std::sync::Arc;

mod error;
mod interleaved_audio;

pub use error::{InterleavedAudioError, RubatoError};
pub use interleaved_audio::{InterleavedAudio, resample};

#[derive(Clone, Debug)]
pub struct AudioClip {
    pub audio: Arc<InterleavedAudio>,
    /// the position of the clip relative to the start of the arrangement
    pub position: ClipPosition,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
}

impl AudioClip {
    #[must_use]
    pub fn create(audio: Arc<InterleavedAudio>, meter: Arc<Meter>) -> Arc<Self> {
        let samples = audio.samples.len();

        Arc::new(Self {
            audio,
            position: ClipPosition::new(
                Position::ZERO,
                Position::from_interleaved_samples(samples, &meter),
                Position::ZERO,
            ),
            meter,
        })
    }

    pub fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        let clip_start_sample = self
            .position
            .get_global_start()
            .in_interleaved_samples(&self.meter);

        let diff = buf_start_sample.abs_diff(clip_start_sample);

        if buf_start_sample > clip_start_sample {
            let start_index = diff
                + self
                    .position
                    .get_clip_start()
                    .in_interleaved_samples(&self.meter);

            if start_index >= self.audio.samples.len() {
                return;
            }

            self.audio.samples[start_index..]
                .iter()
                .zip(buf)
                .for_each(|(sample, buf)| {
                    *buf += sample;
                });
        } else {
            if diff >= buf.len() {
                return;
            }

            self.audio
                .samples
                .iter()
                .zip(buf[diff..].iter_mut())
                .for_each(|(sample, buf)| {
                    *buf += sample;
                });
        }
    }
}
