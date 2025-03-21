use super::error::{InterleavedAudioError, RubatoError};
use generic_daw_utils::NoDebug;
use rubato::{
    Resampler as _, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::{fs::File, path::Path, sync::Arc};
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
    pub lods: NoDebug<Box<[Box<[(f32, f32)]>]>>,
    /// the file name associated with the sample
    pub path: Box<Path>,
}

impl InterleavedAudio {
    pub fn create(path: &Path, sample_rate: u32) -> Result<Arc<Self>, InterleavedAudioError> {
        let samples = Self::read_audio_file(path, sample_rate)?;
        let lods = Self::create_lod(&samples);

        let audio = Self {
            samples: samples.into(),
            lods: lods.into(),
            path: path.into(),
        };

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

    #[expect(clippy::type_complexity)]
    fn create_lod(samples: &[f32]) -> Box<[Box<[(f32, f32)]>]> {
        let mut lods = Vec::with_capacity(10);

        let mut prev = None::<(f32, f32)>;
        lods.push(
            samples
                .chunks(8)
                .map(|chunk| {
                    let (min, max) = chunk.iter().fold(
                        prev.unwrap_or((f32::INFINITY, f32::NEG_INFINITY)),
                        |(min, max), &c| (min.min(c), max.max(c)),
                    );
                    prev = Some((max, min));
                    (min.mul_add(0.5, 0.5), max.mul_add(0.5, 0.5))
                })
                .collect::<Box<_>>(),
        );

        (0..10).for_each(|i| {
            prev = None;
            lods.push(
                lods[i]
                    .chunks(2)
                    .map(|chunk| {
                        let (min, max) = chunk.iter().fold(
                            prev.unwrap_or((f32::INFINITY, f32::NEG_INFINITY)),
                            |(min, max), &c| (min.min(c.0), max.max(c.1)),
                        );
                        prev = Some((max, min));
                        (min, max)
                    })
                    .collect(),
            );
        });

        lods.into_boxed_slice()
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
    let left = planar_samples.next().unwrap();
    let right = planar_samples.next().unwrap();

    Ok(left
        .into_iter()
        .zip(right)
        .flat_map(<[f32; 2]>::from)
        .collect())
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        a %= b;
        (a, b) = (b, a);
    }
    a
}
