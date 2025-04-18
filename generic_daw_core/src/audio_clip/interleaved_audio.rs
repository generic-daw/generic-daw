use crate::Resampler;
use generic_daw_utils::{NoDebug, hash_reader};
use log::info;
use std::{fs::File, hash::DefaultHasher, path::Path, sync::Arc};
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
        let hash = hash_reader::<DefaultHasher>(File::open(&path).unwrap());

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
        debug_assert_eq!(
            hash,
            hash_reader::<DefaultHasher>(File::open(&path).unwrap())
        );

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
        let n_channels = track.codec_params.channels?.count();
        let n_frames = track.codec_params.n_frames? as usize;
        let file_sample_rate = track.codec_params.sample_rate?;
        let delay = track.codec_params.delay.unwrap_or_default() as usize;
        let padding = track.codec_params.padding.unwrap_or_default() as usize;

        let mut samples = Vec::with_capacity(n_frames * n_channels);

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .ok()?;

        let mut sample_buf = None;
        while let Ok(packet) = format.next_packet() {
            if packet.track_id() != track_id {
                continue;
            }

            let audio_buf = decoder.decode(&packet).ok()?;

            let sample_buf = sample_buf.get_or_insert_with(|| {
                let capacity = audio_buf.capacity() as u64;
                let spec = *audio_buf.spec();
                SampleBuffer::new(capacity, spec)
            });

            sample_buf.copy_interleaved_ref(audio_buf.clone());
            samples.extend(sample_buf.samples());
        }

        let samples = &samples[delay * n_channels..];
        let samples = &samples[..samples.len() - padding * n_channels];

        let mut resampler =
            Resampler::new(file_sample_rate as usize, sample_rate as usize, n_channels)?;
        resampler.process(samples);
        Some(resampler.finish().into_boxed_slice())
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
