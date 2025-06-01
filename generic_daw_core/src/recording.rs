use crate::{InterleavedAudio, Meter, Position, Resampler, Stream, build_input_stream};
use async_channel::Receiver;
use cpal::StreamConfig;
use generic_daw_utils::{NoDebug, hash_reader};
use hound::{SampleFormat, WavSpec, WavWriter};
use std::{fs::File, hash::DefaultHasher, path::Path, sync::Arc};

#[derive(Debug)]
pub struct Recording {
    pub lods: NoDebug<Box<[Vec<(f32, f32)>]>>,
    path: Arc<Path>,
    pub name: Arc<str>,
    pub position: Position,

    resampler: Resampler,

    _stream: NoDebug<Stream>,
    config: StreamConfig,
}

impl Recording {
    #[must_use]
    pub fn create(
        path: Arc<Path>,
        meter: &Meter,
        device_name: Option<&str>,
        sample_rate: u32,
        buffer_size: u32,
    ) -> (Self, Receiver<Box<[f32]>>) {
        let (stream, config, receiver) = build_input_stream(device_name, sample_rate, buffer_size);

        let position = Position::from_samples(meter.sample, meter);
        let name = path.file_name().unwrap().to_str().unwrap().into();

        let resampler = Resampler::new(
            config.sample_rate.0 as usize,
            meter.sample_rate as usize,
            config.channels as usize,
        )
        .unwrap();

        (
            Self {
                lods: vec![Vec::new(); 10].into_boxed_slice().into(),
                path,
                name,
                position,

                resampler,

                _stream: stream.into(),
                config,
            },
            receiver,
        )
    }

    pub fn write(&mut self, samples: &[f32]) {
        let start = self.resampler.samples().len();

        self.resampler.process(samples);

        Self::update_lods(self.resampler.samples(), &mut self.lods, start);
    }

    pub fn split_off(&mut self, path: Arc<Path>, meter: &Meter) -> Arc<InterleavedAudio> {
        let mut name = path.file_name().unwrap().to_str().unwrap().into();
        std::mem::swap(&mut self.name, &mut name);
        self.path = path.clone();

        let start = self.resampler.samples().len();

        let mut resampler = Resampler::new(
            self.config.sample_rate.0 as usize,
            meter.sample_rate as usize,
            self.config.channels as usize,
        )
        .unwrap();
        std::mem::swap(&mut self.resampler, &mut resampler);
        let samples = resampler.finish();

        let mut lods = vec![Vec::new(); 10].into_boxed_slice().into();
        std::mem::swap(&mut self.lods, &mut lods);

        Self::update_lods(&samples, &mut lods, start);

        let mut writer = WavWriter::create(
            &path,
            WavSpec {
                channels: self.config.channels,
                sample_rate: self.config.sample_rate.0,
                bits_per_sample: 32,
                sample_format: SampleFormat::Float,
            },
        )
        .unwrap();

        for &s in &samples {
            writer.write_sample(s).unwrap();
        }

        writer.finalize().unwrap();

        let hash = hash_reader::<DefaultHasher>(File::open(&path).unwrap());

        Arc::new(InterleavedAudio {
            samples: samples.into_boxed_slice().into(),
            lods: lods.map(|l| l.into_iter().map(Vec::into_boxed_slice).collect()),
            path,
            name,
            hash,
        })
    }

    #[must_use]
    pub fn finalize(self) -> Arc<InterleavedAudio> {
        let Self {
            mut lods,
            name,
            path,
            resampler,
            ..
        } = self;

        let start = resampler.samples().len();
        let samples = resampler.finish();
        Self::update_lods(&samples, &mut lods, start);

        let mut writer = WavWriter::create(
            &path,
            WavSpec {
                channels: self.config.channels,
                sample_rate: self.config.sample_rate.0,
                bits_per_sample: 32,
                sample_format: SampleFormat::Float,
            },
        )
        .unwrap();

        for &s in &samples {
            writer.write_sample(s).unwrap();
        }

        writer.finalize().unwrap();

        let hash = hash_reader::<DefaultHasher>(File::open(&path).unwrap());

        Arc::new(InterleavedAudio {
            samples: samples.into_boxed_slice().into(),
            lods: lods.map(|l| l.into_iter().map(Vec::into_boxed_slice).collect()),
            path,
            name,
            hash,
        })
    }

    fn update_lods(samples: &[f32], lods: &mut [Vec<(f32, f32)>], mut start: usize) {
        start /= 8;

        lods[0].truncate(start);
        lods[0].extend(samples[start * 8..].chunks(8).map(|chunk| {
            let (min, max) = chunk
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
                    (min.min(c), max.max(c))
                });
            (min.mul_add(0.5, 0.5), max.mul_add(0.5, 0.5))
        }));

        (0..9).for_each(|i| {
            let [last, current] = &mut lods[i..=i + 1] else {
                unreachable!()
            };

            start /= 2;
            current.truncate(start);
            current.extend(last[start * 2..].chunks(2).map(|chunk| {
                chunk
                    .iter()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
                        (min.min(c.0), max.max(c.1))
                    })
            }));
        });
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.resampler.samples().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
