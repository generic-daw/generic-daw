use crate::{InterleavedAudio, Meter, Position, Stream, build_input_stream, resampler};
use async_channel::Receiver;
use generic_daw_utils::{NoDebug, hash_file};
use hound::{SampleFormat, WavSpec, WavWriter};
use rubato::{Resampler as _, SincFixedIn};
use std::{
    fs::File,
    io::BufWriter,
    path::Path,
    sync::{Arc, atomic::Ordering::Acquire},
};

#[derive(Debug)]
pub struct Recording {
    /// these are used to play the sample back
    pub(crate) samples: NoDebug<Vec<f32>>,
    /// these are used to draw the sample in various quality levels
    pub lods: NoDebug<Box<[Vec<(f32, f32)>]>>,
    /// the file path associated with the sample
    pub path: Arc<Path>,
    /// the file name associated with the sample
    pub name: Arc<str>,

    writer: NoDebug<WavWriter<BufWriter<File>>>,
    pub position: Position,
    channels: usize,
    sample_rate: u32,
    buffer_size: usize,

    resampler: Option<NoDebug<SincFixedIn<f32>>>,
    resample_buffer_in: [Vec<f32>; 2],
    resample_buffer_out: [Vec<f32>; 2],

    _stream: NoDebug<Stream>,
}

impl Recording {
    pub fn create(path: Arc<Path>, meter: &Meter) -> (Self, Receiver<Box<[f32]>>) {
        let (stream, config, receiver) = build_input_stream(meter.sample_rate, meter.buffer_size);

        let start_pos = Position::from_samples(
            meter.sample.load(Acquire),
            meter.bpm.load(Acquire),
            meter.sample_rate,
        );

        let writer = WavWriter::create(
            path.as_ref(),
            WavSpec {
                channels: config.channels,
                sample_rate: config.sample_rate.0,
                bits_per_sample: 32,
                sample_format: SampleFormat::Float,
            },
        )
        .unwrap()
        .into();

        let name = path.file_name().unwrap().to_str().unwrap().into();

        let resampler = resampler(meter.sample_rate, config.sample_rate.0, meter.buffer_size)
            .map(|x| x.unwrap().into());

        (
            Self {
                samples: Vec::new().into(),
                lods: vec![Vec::new(); 10].into_boxed_slice().into(),
                path,
                name,

                writer,
                position: start_pos,
                channels: config.channels as usize,
                sample_rate: config.sample_rate.0,
                buffer_size: meter.buffer_size as usize,

                resampler,
                resample_buffer_in: [Vec::new(), Vec::new()],
                resample_buffer_out: [Vec::new(), Vec::new()],

                _stream: stream.into(),
            },
            receiver,
        )
    }

    pub fn write(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.writer.write_sample(sample).unwrap();
        }

        let mut start = self.samples.len() / 8;

        if let Some(resampler) = self.resampler.as_mut() {
            let [in_l, in_r] = &mut self.resample_buffer_in;
            let [out_l, out_r] = &mut self.resample_buffer_out;

            in_l.extend(samples.iter().step_by(self.channels));
            in_r.extend(
                samples
                    .iter()
                    .skip(usize::from(self.channels != 1))
                    .step_by(self.channels),
            );

            while in_l.len() >= self.buffer_size {
                resampler
                    .process_into_buffer(&[&in_l, &in_r], &mut [&mut *out_l, &mut *out_r], None)
                    .unwrap();

                self.samples
                    .extend(out_l.iter().zip(&*out_r).flat_map(<[&f32; 2]>::from));

                out_l.clear();
                out_r.clear();

                in_l.rotate_left(self.buffer_size);
                in_l.truncate(in_l.len() - self.buffer_size);
                in_r.rotate_left(self.buffer_size);
                in_r.truncate(in_r.len() - self.buffer_size);
            }
        } else {
            self.samples.extend(
                samples
                    .iter()
                    .step_by(self.channels)
                    .zip(
                        samples
                            .iter()
                            .skip(usize::from(self.channels != 1))
                            .step_by(self.channels),
                    )
                    .flat_map(<[&f32; 2]>::from),
            );
        }

        self.lods[0].truncate(start);
        self.lods[0].extend(self.samples[start * 8..].chunks(8).map(|chunk| {
            let (min, max) = chunk
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
                    (min.min(c), max.max(c))
                });
            (min.mul_add(0.5, 0.5), max.mul_add(0.5, 0.5))
        }));

        (0..9).for_each(|i| {
            let [last, current] = self.lods.get_mut(i..=i + 1).unwrap() else {
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

    pub fn split_off(&mut self, path: Arc<Path>) -> Arc<InterleavedAudio> {
        let mut writer = WavWriter::create(
            path.as_ref(),
            WavSpec {
                channels: self.channels as u16,
                sample_rate: self.sample_rate,
                bits_per_sample: 32,
                sample_format: SampleFormat::Float,
            },
        )
        .unwrap();
        std::mem::swap(&mut self.writer.0, &mut writer);
        writer.finalize().unwrap();

        let mut samples = Vec::new().into();
        std::mem::swap(&mut self.samples, &mut samples);

        let mut lods = vec![Vec::new(); 10].into_boxed_slice().into();
        std::mem::swap(&mut self.lods, &mut lods);

        let mut name = path.as_ref().file_name().unwrap().to_str().unwrap().into();
        std::mem::swap(&mut self.name, &mut name);
        self.path = path.as_ref().into();

        let hash = hash_file(&path);

        Arc::new(InterleavedAudio {
            samples: samples.map(Vec::into_boxed_slice),
            lods: lods.map(|l| l.into_iter().map(Vec::into_boxed_slice).collect()),
            path,
            name,
            hash,
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl TryFrom<Recording> for Arc<InterleavedAudio> {
    type Error = hound::Error;

    fn try_from(value: Recording) -> Result<Self, Self::Error> {
        let Recording {
            samples,
            lods,
            name,
            path,
            writer,
            ..
        } = value;

        writer.0.finalize()?;
        let hash = hash_file(&path);

        Ok(Self::new(InterleavedAudio {
            samples: samples.map(Vec::into_boxed_slice),
            lods: lods.map(|l| l.into_iter().map(Vec::into_boxed_slice).collect()),
            path,
            name,
            hash,
        }))
    }
}
