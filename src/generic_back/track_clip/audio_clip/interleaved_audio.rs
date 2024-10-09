use crate::{generic_back::Arrangement, helpers::gcd};
use anyhow::{anyhow, Result};
use itertools::{
    Itertools,
    MinMaxResult::{MinMax, NoElements, OneElement},
};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::{
    cmp::{max_by, min_by},
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

#[derive(Debug)]
pub struct InterleavedAudio {
    /// these are used to play the sample back
    pub samples: Vec<f32>,
    /// these are used to draw the sample in various quality levels
    pub lods: RwLock<[Vec<(f32, f32)>; 10]>,
    /// the file name associated with the sample
    pub name: PathBuf,
}

impl InterleavedAudio {
    pub fn create(path: PathBuf, arrangement: &Arc<Arrangement>) -> Result<Arc<Self>> {
        let mut samples = Self::read_audio_file(&path, arrangement)?;
        samples.shrink_to_fit();

        let length = samples.len();
        let audio = Arc::new(Self {
            samples,
            lods: RwLock::new([
                vec![(0.0, 0.0); length.div_ceil(1 << 3)],
                vec![(0.0, 0.0); length.div_ceil(1 << 4)],
                vec![(0.0, 0.0); length.div_ceil(1 << 5)],
                vec![(0.0, 0.0); length.div_ceil(1 << 6)],
                vec![(0.0, 0.0); length.div_ceil(1 << 7)],
                vec![(0.0, 0.0); length.div_ceil(1 << 8)],
                vec![(0.0, 0.0); length.div_ceil(1 << 9)],
                vec![(0.0, 0.0); length.div_ceil(1 << 10)],
                vec![(0.0, 0.0); length.div_ceil(1 << 11)],
                vec![(0.0, 0.0); length.div_ceil(1 << 12)],
            ]),
            name: path,
        });

        Self::create_lod(&audio);
        Ok(audio)
    }

    fn read_audio_file(path: &PathBuf, arrangement: &Arc<Arrangement>) -> Result<Vec<f32>> {
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

            match decoder.decode(&packet) {
                Ok(audio_buf) => {
                    if sample_buffer.is_none() {
                        let spec = *audio_buf.spec();
                        let duration = u64::try_from(audio_buf.capacity()).unwrap();
                        sample_buffer = Some(SampleBuffer::<f32>::new(duration, spec));
                    }
                    if let Some(buf) = &mut sample_buffer {
                        buf.copy_interleaved_ref(audio_buf);
                        interleaved_samples.extend(buf.samples().iter());
                    }
                }
                Err(err) => return Err(anyhow!(err)),
            }
        }

        let stream_sample_rate = arrangement.meter.sample_rate.load(SeqCst);

        resample(file_sample_rate, stream_sample_rate, interleaved_samples)
    }

    fn create_lod(audio: &Arc<Self>) {
        audio.samples.chunks(8).enumerate().for_each(|(i, chunk)| {
            let (min, max) = match chunk.iter().minmax_by(|a, b| a.partial_cmp(b).unwrap()) {
                MinMax(min, max) => (min, max),
                OneElement(x) => (x, x),
                NoElements => unreachable!(),
            };
            audio.lods.write().unwrap()[0][i] =
                ((*min).mul_add(0.5, 0.5), (*max).mul_add(0.5, 0.5));
        });

        (1..10).for_each(|i| {
            let len = audio.lods.read().unwrap()[i].len();
            (0..len).for_each(|j| {
                let min = min_by(
                    audio.lods.read().unwrap()[i - 1][2 * j].0,
                    audio.lods.read().unwrap()[i - 1]
                        .get(2 * j + 1)
                        .unwrap_or(&(f32::MAX, f32::MAX))
                        .0,
                    |a, b| a.partial_cmp(b).unwrap(),
                );
                let max = max_by(
                    audio.lods.read().unwrap()[i - 1][2 * j].1,
                    audio.lods.read().unwrap()[i - 1]
                        .get(2 * j + 1)
                        .unwrap_or(&(f32::MAX, f32::MAX))
                        .1,
                    |a, b| a.partial_cmp(b).unwrap(),
                );
                audio.lods.write().unwrap()[i][j] = (min, max);
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

    let frames = interleaved_samples.len() / 2;
    let mut resampler = SincFixedIn::<f32>::new(
        resample_ratio,
        1.0,
        SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Nearest,
            oversampling_factor: usize::try_from(
                file_sample_rate / gcd(stream_sample_rate, file_sample_rate),
            )
            .unwrap(),
            window: WindowFunction::Blackman,
        },
        frames,
        2,
    )?;

    let mut left = Vec::with_capacity(frames);
    let mut right = Vec::with_capacity(frames);
    interleaved_samples
        .iter()
        .enumerate()
        .for_each(|(i, &sample)| {
            if i % 2 == 0 {
                left.push(sample);
            } else {
                right.push(sample);
            }
        });

    let deinterleaved_samples = resampler.process(&[left, right], None)?;

    let frames = deinterleaved_samples[0].len();
    interleaved_samples.clear();
    interleaved_samples.reserve_exact(frames * 2);
    let left = &deinterleaved_samples[0];
    let right = &deinterleaved_samples[1];
    for i in 0..frames {
        interleaved_samples.push(left[i]);
        interleaved_samples.push(right[i]);
    }

    Ok(interleaved_samples)
}
