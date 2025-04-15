use generic_daw_utils::{NoDebug, hash_file};
use log::info;
use rubato::{
    Resampler as _, SincFixedIn, SincInterpolationParameters, SincInterpolationType,
    WindowFunction, calculate_cutoff,
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

#[derive(Debug)]
pub struct InterleavedAudio {
    /// these are used to play the sample back
    pub(crate) samples: NoDebug<Box<[f32]>>,
    /// these are used to draw the sample in various quality levels
    pub lods: NoDebug<Box<[Box<[(f32, f32)]>]>>,
    /// the file path associated with the sample
    pub path: Arc<Path>,
    /// the file name associated with the sample
    pub name: Arc<str>,
    /// the hash of the file associated with the sample
    pub hash: u64,
}

impl InterleavedAudio {
    #[must_use]
    pub fn create(path: Arc<Path>, sample_rate: u32) -> Option<Arc<Self>> {
        info!("loading sample {path:?}");

        let name = path.as_ref().file_name()?.to_str()?.into();
        let samples = Self::read_audio_file(&path, sample_rate)?;
        let lods = Self::create_lod(&samples);
        let hash = hash_file(&path);

        info!("loaded sample {path:?}");

        Some(Arc::new(Self {
            samples: samples.into(),
            lods: lods.into(),
            path,
            name,
            hash,
        }))
    }

    #[must_use]
    pub fn create_with_hash(path: Arc<Path>, sample_rate: u32, hash: u64) -> Option<Arc<Self>> {
        info!("loading sample {path:?}");

        let name = path.as_ref().file_name()?.to_str()?.into();
        let samples = Self::read_audio_file(&path, sample_rate)?;
        let lods = Self::create_lod(&samples);
        debug_assert_eq!(hash, hash_file(&path));

        info!("loaded sample {path:?}");

        Some(Arc::new(Self {
            samples: samples.into(),
            lods: lods.into(),
            path,
            name,
            hash,
        }))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn read_audio_file(path: impl AsRef<Path>, sample_rate: u32) -> Option<Box<[f32]>> {
        let mut format = symphonia::default::get_probe()
            .format(
                &Hint::default(),
                MediaSourceStream::new(
                    Box::new(File::open(path).ok()?),
                    MediaSourceStreamOptions::default(),
                ),
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .ok()?
            .format;

        let track = format.default_track()?;
        let track_id = track.id;
        let n_frames = track.codec_params.n_frames? as usize;
        let file_sample_rate = track.codec_params.sample_rate?;

        let mut left = Vec::with_capacity(n_frames);
        let mut right = Vec::with_capacity(n_frames);

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .ok()?;

        let mut sample_buffer = None;
        while let Ok(packet) = format.next_packet() {
            if packet.track_id() != track_id {
                continue;
            }

            let audio_buf = decoder.decode(&packet).ok()?;

            let buf = sample_buffer.get_or_insert_with(|| {
                let duration = audio_buf.capacity() as u64;
                let spec = *audio_buf.spec();
                SampleBuffer::new(duration, spec)
            });

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

        resample_planar(file_sample_rate, sample_rate, left, right)
    }

    fn create_lod(samples: &[f32]) -> Box<[Box<[(f32, f32)]>]> {
        let mut lods = Vec::with_capacity(10);

        lods.push(
            samples
                .chunks(8)
                .map(|chunk| {
                    let (min, max) = chunk
                        .iter()
                        .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
                            (min.min(c), max.max(c))
                        });
                    (min.mul_add(0.5, 0.5), max.mul_add(0.5, 0.5))
                })
                .collect::<Box<_>>(),
        );

        (0..10).for_each(|i| {
            lods.push(
                lods[i]
                    .chunks(2)
                    .map(|chunk| {
                        chunk
                            .iter()
                            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
                                (min.min(c.0), max.max(c.1))
                            })
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
) -> Option<Box<[f32]>> {
    if file_sample_rate == stream_sample_rate {
        return Some(interleaved_samples.into_boxed_slice());
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

fn resample_planar(
    file_sample_rate: u32,
    stream_sample_rate: u32,
    left: Vec<f32>,
    right: Vec<f32>,
) -> Option<Box<[f32]>> {
    let Some(resampler) = resampler(file_sample_rate, stream_sample_rate, left.len() as u32) else {
        return Some(
            left.into_iter()
                .zip(right)
                .flat_map(<[f32; 2]>::from)
                .collect(),
        );
    };

    let mut planar_samples = resampler?.process(&[&left, &right], None).ok()?.into_iter();
    let left = planar_samples.next().unwrap();
    let right = planar_samples.next().unwrap();

    Some(
        left.into_iter()
            .zip(right)
            .flat_map(<[f32; 2]>::from)
            .collect(),
    )
}

#[expect(clippy::option_option)]
pub fn resampler(
    file_sample_rate: u32,
    stream_sample_rate: u32,
    chunk_size: u32,
) -> Option<Option<SincFixedIn<f32>>> {
    (file_sample_rate != stream_sample_rate).then_some(
        SincFixedIn::new(
            f64::from(stream_sample_rate) / f64::from(file_sample_rate),
            1.0,
            SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: calculate_cutoff(256, WindowFunction::BlackmanHarris2),
                interpolation: SincInterpolationType::Cubic,
                oversampling_factor: 256,
                window: WindowFunction::BlackmanHarris2,
            },
            chunk_size as usize,
            2,
        )
        .ok(),
    )
}
