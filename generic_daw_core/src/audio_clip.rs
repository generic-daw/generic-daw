use crate::{ClipPosition, Meter, Position};
use std::sync::Arc;

mod interleaved_audio;

pub use interleaved_audio::InterleavedAudio;

#[derive(Clone, Debug)]
pub struct AudioClip {
    pub audio: Arc<InterleavedAudio>,
    /// the position of the clip relative to the start of the arrangement
    pub position: ClipPosition,
}

impl AudioClip {
    #[must_use]
    pub fn create(audio: Arc<InterleavedAudio>, meter: &Meter) -> Arc<Self> {
        let samples = audio.samples.len();

        Arc::new(Self {
            audio,
            position: ClipPosition::new(
                Position::ZERO,
                Position::from_samples(samples, meter),
                Position::ZERO,
            ),
        })
    }

    pub fn process(&self, meter: &Meter, audio: &mut [f32]) {
        if !meter.playing {
            return;
        }

        let clip_start_sample = self.position.get_global_start().in_samples(meter);
        let diff = meter.sample.abs_diff(clip_start_sample);

        if meter.sample > clip_start_sample {
            let start_index = diff + self.position.get_clip_start().in_samples(meter);

            if start_index >= self.audio.samples.len() {
                return;
            }

            self.audio.samples[start_index..]
                .iter()
                .zip(audio)
                .for_each(|(sample, buf)| {
                    *buf += sample;
                });
        } else {
            if diff >= audio.len() {
                return;
            }

            self.audio
                .samples
                .iter()
                .zip(audio[diff..].iter_mut())
                .for_each(|(sample, buf)| {
                    *buf += sample;
                });
        }
    }
}
