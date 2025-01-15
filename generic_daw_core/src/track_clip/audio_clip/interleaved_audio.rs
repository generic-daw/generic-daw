use crate::Meter;
use anyhow::Result;
use itertools::{Itertools as _, MinMaxResult};
use rubato::{
    Resampler as _, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::{
    array,
    cmp::{max_by, min_by},
    fmt::Debug,
    fs::File,
    path::PathBuf,
    sync::{atomic::Ordering::SeqCst, Arc, RwLock},
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    formats::FormatOptions,
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
    probe::Hint,
};

#[expect(clippy::type_complexity)]
pub struct InterleavedAudio {
    /// these are used to play the sample back
    pub(crate) samples: Box<[f32]>,
    /// these are used to draw the sample in various quality levels
    pub lods: [RwLock<Box<[(f32, f32)]>>; 10],
    /// the file name associated with the sample
    pub(crate) path: PathBuf,
}

impl Debug for InterleavedAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InterleavedAudio")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl InterleavedAudio {
    pub fn create(path: PathBuf, meter: &Meter) -> Result<Arc<Self>> {
        let samples = Self::read_audio_file(&path, meter)?;
        let length = samples.len();

        let audio = Arc::new(Self {
            samples,
            lods: array::from_fn(|i| {
                RwLock::new(vec![(0.0, 0.0); length.div_ceil(1 << (i + 3))].into_boxed_slice())
            }),
            path,
        });

        Self::create_lod(&audio);
        Ok(audio)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn read_audio_file(path: &PathBuf, meter: &Meter) -> Result<Box<[f32]>> {
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
                let spec = *audio_buf.spec();
                let duration = audio_buf.frames() as u64;
                sample_buffer.replace(SampleBuffer::new(duration, spec));
                sample_buffer.as_mut().unwrap()
            };

            buf.copy_interleaved_ref(audio_buf);
            interleaved_samples.extend(buf.samples());
        }

        let stream_sample_rate = meter.sample_rate.load(SeqCst);

        resample(file_sample_rate, stream_sample_rate, interleaved_samples)
            .map(Vec::into_boxed_slice)
    }

    fn create_lod(audio: &Self) {
        audio.samples.chunks(8).enumerate().for_each(|(i, chunk)| {
            let (min, max) = match chunk.iter().minmax_by(|a, b| a.partial_cmp(b).unwrap()) {
                MinMaxResult::MinMax(min, max) => (min, max),
                MinMaxResult::OneElement(x) => (x, x),
                MinMaxResult::NoElements => unreachable!(),
            };
            audio.lods[0].write().unwrap()[i] = (min.mul_add(0.5, 0.5), max.mul_add(0.5, 0.5));
        });

        (1..10).for_each(|i| {
            let len = audio.lods[i].read().unwrap().len();
            (0..len).for_each(|j| {
                let min = min_by(
                    audio.lods[i - 1].read().unwrap()[2 * j].0,
                    audio.lods[i - 1]
                        .read()
                        .unwrap()
                        .get(2 * j + 1)
                        .unwrap_or(&(f32::MAX, f32::MAX))
                        .0,
                    |a, b| a.partial_cmp(b).unwrap(),
                );
                let max = max_by(
                    audio.lods[i - 1].read().unwrap()[2 * j].1,
                    audio.lods[i - 1]
                        .read()
                        .unwrap()
                        .get(2 * j + 1)
                        .unwrap_or(&(f32::MAX, f32::MAX))
                        .1,
                    |a, b| a.partial_cmp(b).unwrap(),
                );
                audio.lods[i].write().unwrap()[j] = (min, max);
            });
        });
    }
}

pub fn resample(
    file_sample_rate: u32,
    stream_sample_rate: u32,
    mut interleaved_samples: Vec<f32>,
) -> Result<Vec<f32>> {
    if file_sample_rate == stream_sample_rate {
        return Ok(interleaved_samples);
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
        .step_by(2)
        .copied()
        .collect::<Box<_>>();
    let right = interleaved_samples
        .iter()
        .skip(1)
        .step_by(2)
        .copied()
        .collect();

    let deinterleaved_samples = resampler.process(&[left, right], None)?;

    interleaved_samples.clear();
    interleaved_samples.extend(
        deinterleaved_samples[0]
            .iter()
            .interleave(&deinterleaved_samples[1]),
    );

    Ok(interleaved_samples)
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        a %= b;
        (a, b) = (b, a);
    }
    a
}
