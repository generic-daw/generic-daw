use super::error::{InterleavedAudioError, RubatoError};
use crate::Meter;
use rubato::{
    Resampler as _, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::{
    array,
    cmp::{max_by, min_by},
    fmt::{Debug, Formatter},
    fs::File,
    path::PathBuf,
    sync::Arc,
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    formats::FormatOptions,
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
    probe::Hint,
};

pub struct InterleavedAudio {
    /// these are used to play the sample back
    pub samples: Box<[f32]>,
    /// these are used to draw the sample in various quality levels
    pub lods: [Box<[(f32, f32)]>; 10],
    /// the file name associated with the sample
    pub path: PathBuf,
}

impl Debug for InterleavedAudio {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InterleavedAudio")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl InterleavedAudio {
    pub fn create(path: PathBuf, meter: &Meter) -> Result<Arc<Self>, InterleavedAudioError> {
        let samples = Self::read_audio_file(&path, meter)?;
        let length = samples.len();

        let mut audio = Self {
            samples,
            lods: array::from_fn(|i| {
                vec![(0.0, 0.0); length.div_ceil(1 << (i + 3))].into_boxed_slice()
            }),
            path,
        };

        audio.create_lod();

        Ok(Arc::new(audio))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn read_audio_file(path: &PathBuf, meter: &Meter) -> Result<Box<[f32]>, InterleavedAudioError> {
        let mut format = symphonia::default::get_probe()
            .format(
                &Hint::default(),
                MediaSourceStream::new(
                    Box::new(File::open(path).unwrap()),
                    MediaSourceStreamOptions::default(),
                ),
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )?
            .format;

        let track = format.default_track().unwrap();
        let track_id = track.id;
        let file_sample_rate = track.codec_params.sample_rate.unwrap();

        let mut interleaved_samples =
            Vec::with_capacity(track.codec_params.n_frames.unwrap() as usize * 2);

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())?;

        let mut sample_buffer = None;
        while let Ok(packet) = format.next_packet() {
            if packet.track_id() != track_id {
                continue;
            }

            let audio_buf = decoder.decode(&packet)?;

            let buf = if let Some(buf) = &mut sample_buffer {
                buf
            } else {
                let duration = audio_buf.capacity() as u64;
                let spec = *audio_buf.spec();
                sample_buffer.replace(SampleBuffer::new(duration, spec));
                sample_buffer.as_mut().unwrap()
            };

            buf.copy_interleaved_ref(audio_buf);
            interleaved_samples.extend(buf.samples());
        }

        Ok(resample(
            file_sample_rate,
            meter.sample_rate,
            interleaved_samples,
        )?)
    }

    fn create_lod(&mut self) {
        self.samples.chunks(8).enumerate().for_each(|(i, chunk)| {
            let (min, max) = chunk
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
                    (min.min(c), max.max(c))
                });
            self.lods[0][i] = (min.mul_add(0.5, 0.5), max.mul_add(0.5, 0.5));
        });

        (1..self.lods.len()).for_each(|i| {
            let len = self.lods[i].len();
            (0..len).for_each(|j| {
                let min = min_by(
                    self.lods[i - 1][2 * j].0,
                    self.lods[i - 1]
                        .get(2 * j + 1)
                        .unwrap_or(&(f32::INFINITY, f32::INFINITY))
                        .0,
                    f32::total_cmp,
                );
                let max = max_by(
                    self.lods[i - 1][2 * j].1,
                    self.lods[i - 1]
                        .get(2 * j + 1)
                        .unwrap_or(&(f32::INFINITY, f32::INFINITY))
                        .1,
                    f32::total_cmp,
                );
                self.lods[i][j] = (min, max);
            });
        });
    }
}

pub fn resample(
    file_sample_rate: u32,
    stream_sample_rate: u32,
    mut interleaved_samples: Vec<f32>,
) -> Result<Box<[f32]>, RubatoError> {
    if file_sample_rate == stream_sample_rate {
        return Ok(interleaved_samples.into_boxed_slice());
    }

    let resample_ratio = f64::from(stream_sample_rate) / f64::from(file_sample_rate);
    let oversampling_factor =
        (file_sample_rate / gcd(stream_sample_rate, file_sample_rate)) as usize;

    let mut resampler = SincFixedIn::new(
        resample_ratio,
        1.0,
        SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Nearest,
            oversampling_factor,
            window: WindowFunction::Blackman,
        },
        interleaved_samples.len() / 2,
        2,
    )?;

    let left = interleaved_samples
        .iter()
        .copied()
        .step_by(2)
        .collect::<Box<_>>();
    let right = interleaved_samples
        .iter()
        .copied()
        .skip(1)
        .step_by(2)
        .collect();

    let deinterleaved_samples = resampler.process(&[left, right], None)?;

    interleaved_samples.clear();
    interleaved_samples.extend(
        deinterleaved_samples[0]
            .iter()
            .zip(&deinterleaved_samples[1])
            .flat_map(<[&f32; 2]>::from),
    );

    Ok(interleaved_samples.into_boxed_slice())
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        a %= b;
        (a, b) = (b, a);
    }
    a
}
