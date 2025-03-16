use super::error::{InterleavedAudioError, RubatoError};
use generic_daw_utils::NoDebug;
use rubato::{
    Resampler as _, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::{
    array,
    cmp::{max_by, min_by},
    fs::File,
    path::Path,
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

#[expect(clippy::type_complexity)]
#[derive(Debug)]
pub struct InterleavedAudio {
    /// these are used to play the sample back
    pub samples: NoDebug<Box<[f32]>>,
    /// these are used to draw the sample in various quality levels
    pub lods: NoDebug<[Box<[(f32, f32)]>; 10]>,
    /// the file name associated with the sample
    pub path: Box<Path>,
}

impl InterleavedAudio {
    pub fn create(path: &Path, sample_rate: u32) -> Result<Arc<Self>, InterleavedAudioError> {
        let samples = Self::read_audio_file(path, sample_rate)?;
        let length = samples.len();

        let mut audio = Self {
            samples: samples.into(),
            lods: array::from_fn(|i| {
                vec![(0.0, 0.0); length.div_ceil(1 << (i + 3))].into_boxed_slice()
            })
            .into(),
            path: Box::from(path),
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

    fn read_audio_file(path: &Path, sample_rate: u32) -> Result<Box<[f32]>, InterleavedAudioError> {
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
        let n_frames = track.codec_params.n_frames.unwrap() as usize;
        let file_sample_rate = track.codec_params.sample_rate.unwrap();

        let mut left = Vec::with_capacity(n_frames);
        let mut right = Vec::with_capacity(n_frames);

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

            buf.copy_planar_ref(audio_buf.clone());

            if audio_buf.spec().channels.count() == 1 {
                left.extend(buf.samples());
                right.extend(buf.samples());
            } else {
                let l = &buf.samples()[..audio_buf.frames()];
                let r = &buf.samples()[audio_buf.frames()..][..audio_buf.frames()];
                left.extend(l);
                right.extend(r);
            }
        }

        Ok(resample_planar(file_sample_rate, sample_rate, left, right)?)
    }

    fn create_lod(&mut self) {
        let mut prev = None::<(f32, f32)>;
        self.samples.chunks(8).enumerate().for_each(|(i, chunk)| {
            let (mut min, mut max) = chunk
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
                    (min.min(c), max.max(c))
                });
            if let Some(prev) = prev {
                min = min.min(prev.1);
                max = max.max(prev.0);
            }
            if max - min < 0.02 {
                let avg = min.midpoint(max).clamp(-0.99, 0.99);
                (min, max) = (avg - 0.01, avg + 0.01);
            }
            prev = Some((min, max));
            self.lods[0][i] = (min.mul_add(0.5, 0.5), max.mul_add(0.5, 0.5));
        });

        (1..self.lods.len()).for_each(|i| {
            prev = None;
            let len = self.lods[i].len();
            (0..len).for_each(|j| {
                let mut min = min_by(
                    self.lods[i - 1][2 * j].0,
                    self.lods[i - 1]
                        .get(2 * j + 1)
                        .unwrap_or(&(f32::INFINITY, f32::INFINITY))
                        .0,
                    f32::total_cmp,
                );
                let mut max = max_by(
                    self.lods[i - 1][2 * j].1,
                    self.lods[i - 1]
                        .get(2 * j + 1)
                        .unwrap_or(&(f32::INFINITY, f32::INFINITY))
                        .1,
                    f32::total_cmp,
                );
                if let Some(prev) = prev {
                    min = min.min(prev.1);
                    max = max.max(prev.0);
                }
                prev = Some((min, max));
                self.lods[i][j] = (min, max);
            });
        });
    }
}

pub fn resample_interleaved(
    file_sample_rate: u32,
    stream_sample_rate: u32,
    interleaved_samples: Vec<f32>,
) -> Result<Box<[f32]>, RubatoError> {
    if file_sample_rate == stream_sample_rate {
        return Ok(interleaved_samples.into_boxed_slice());
    }

    let left = interleaved_samples
        .iter()
        .copied()
        .step_by(2)
        .collect::<Vec<_>>();

    let mut right = interleaved_samples;
    let mut keep = true;
    right.retain(|_| {
        keep ^= true;
        keep
    });

    resample_planar(file_sample_rate, stream_sample_rate, left, right)
}

pub fn resample_planar(
    file_sample_rate: u32,
    stream_sample_rate: u32,
    left: Vec<f32>,
    right: Vec<f32>,
) -> Result<Box<[f32]>, RubatoError> {
    if file_sample_rate == stream_sample_rate {
        return Ok(left
            .into_iter()
            .zip(right)
            .flat_map(<[f32; 2]>::from)
            .collect());
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
        left.len(),
        2,
    )?;

    let mut planar_samples = resampler.process(&[&left, &right], None)?.into_iter();
    let l = planar_samples.next().unwrap();
    let r = planar_samples.next().unwrap();

    let mut interleaved_samples = left;
    interleaved_samples.clear();
    interleaved_samples.extend(l.into_iter().zip(r).flat_map(<[f32; 2]>::from));

    Ok(interleaved_samples.into_boxed_slice())
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        a %= b;
        (a, b) = (b, a);
    }
    a
}
