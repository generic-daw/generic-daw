use anyhow::{anyhow, Result};
use rubato::{
    SincFixedIn, SincInterpolationParameters, SincInterpolationType, VecResampler, WindowFunction,
};
use std::{fs::File, path::PathBuf, sync::Arc};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    errors::Error,
    formats::FormatOptions,
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
    probe::Hint,
};

use crate::generic_back::position::{Meter, Position};

use super::TrackClip;

pub struct InterleavedAudio {
    samples: Arc<[f32]>,
}

impl InterleavedAudio {
    pub fn new(samples: Arc<[f32]>) -> Self {
        Self { samples }
    }

    pub fn len(&self) -> u32 {
        u32::try_from(self.samples.len()).unwrap()
    }

    pub fn get_sample_at_index(&self, index: u32) -> &f32 {
        self.samples.get(index as usize).unwrap_or(&0.0)
    }
}

pub struct AudioClip {
    audio: Arc<InterleavedAudio>,
    global_start: Position,
    global_end: Position,
    clip_start: Position,
    volume: f32,
}

impl AudioClip {
    pub fn new(audio: Arc<InterleavedAudio>, meter: &Meter) -> Self {
        let samples = audio.len();
        Self {
            audio,
            global_start: Position::new(0, 0),
            global_end: Position::from_interleaved_samples(samples, meter),
            clip_start: Position::new(0, 0),
            volume: 1.0,
        }
    }
}

impl TrackClip for AudioClip {
    fn get_at_global_time(&self, global_time: u32, meter: &Meter) -> f32 {
        self.audio.get_sample_at_index(
            global_time - (self.global_start + self.clip_start).in_interleaved_samples(meter),
        ) * self.volume
    }

    fn get_global_start(&self) -> Position {
        self.global_start
    }

    fn get_global_end(&self) -> Position {
        self.global_end
    }

    fn trim_start_to(&mut self, clip_start: Position) {
        self.clip_start = clip_start;
    }

    fn trim_end_to(&mut self, global_end: Position) {
        self.global_end = global_end;
    }

    fn move_start_to(&mut self, global_start: Position) {
        match self.global_start.cmp(&global_start) {
            std::cmp::Ordering::Less => {
                self.global_end += global_start - self.global_start;
            }
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Greater => {
                self.global_end += self.global_start - global_start;
            }
        }
        self.global_start = global_start;
    }
}

pub fn read_audio_file(path: &PathBuf, meter: &Meter) -> Result<Arc<InterleavedAudio>> {
    let mut samples = Vec::new();

    let format = symphonia::default::get_probe().format(
        &Hint::new(),
        MediaSourceStream::new(
            Box::new(File::open(path).unwrap()),
            MediaSourceStreamOptions::default(),
        ),
        &FormatOptions::default(),
        &MetadataOptions::default(),
    );

    if let Err(err) = format {
        return Err(anyhow!(err));
    }

    let mut format = format.unwrap().format;

    let track = format.default_track().unwrap();
    let sample_rate = track.codec_params.sample_rate.unwrap();

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .unwrap();

    let track_id = track.id;

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
                    samples.extend(buf.samples().iter());
                }
            }
            Err(Error::DecodeError(_)) => (),
            Err(_) => break,
        }
    }

    if sample_rate == meter.sample_rate {
        return Ok(Arc::new(InterleavedAudio::new(samples.into())));
    }

    let mut resampler = SincFixedIn::<f32>::new(
        f64::from(meter.sample_rate) / f64::from(sample_rate),
        2.0,
        SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::Blackman,
        },
        samples.len() / 2,
        2,
    )
    .unwrap();

    let deinterleaved_samples: Vec<Vec<f32>> = vec![
        samples.iter().step_by(2).copied().collect(),
        samples.iter().skip(1).step_by(2).copied().collect(),
    ];
    assert_eq!(
        deinterleaved_samples[0].len(),
        deinterleaved_samples[1].len()
    );

    let resampled_file = resampler.process(&deinterleaved_samples, None).unwrap();

    samples.clear();
    for i in 0..resampled_file[0].len() {
        samples.extend(resampled_file.iter().map(|s| s[i]));
    }

    Ok(Arc::new(InterleavedAudio::new(samples.into())))
}
