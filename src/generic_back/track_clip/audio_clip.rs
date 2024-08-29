use super::TrackClip;
use anyhow::{anyhow, Result};
use cpal::StreamConfig;
use rubato::{
    SincFixedIn, SincInterpolationParameters, SincInterpolationType, VecResampler, WindowFunction,
};
use std::{cmp::min, fs::File, path::PathBuf, sync::Arc};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    errors::Error,
    formats::FormatOptions,
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
    probe::Hint,
};

pub struct InterleavedAudio {
    samples: Arc<[f32]>,
    name: String,
}

impl InterleavedAudio {
    pub fn new(samples: Arc<[f32]>, name: String) -> Self {
        Self { samples, name }
    }

    pub fn len(&self) -> u32 {
        self.samples.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_sample_at_index(&self, index: u32) -> &f32 {
        self.samples.get(index as usize).unwrap_or(&0.0)
    }
}

pub struct AudioClip {
    audio: Arc<InterleavedAudio>,
    global_start: u32,
    global_end: u32,
    clip_start: u32,
    volume: f32,
}

impl TrackClip for AudioClip {
    fn get_at_global_time(&self, global_time: u32) -> f32 {
        self.audio
            .get_sample_at_index(global_time - self.global_start + self.clip_start)
            * self.volume
    }

    fn get_global_start(&self) -> u32 {
        self.global_start
    }

    fn get_global_end(&self) -> u32 {
        self.global_end
    }
}

impl AudioClip {
    pub fn new(audio: Arc<InterleavedAudio>) -> Self {
        let global_end = audio.len();
        Self {
            audio,
            global_start: 0,
            global_end,
            clip_start: 0,
            volume: 1.0,
        }
    }

    pub fn trim_start(&mut self, samples: i32) {
        if samples < 0 {
            let samples = -samples as u32;
            let samples = min(samples, self.global_start);
            let samples = min(samples, self.clip_start);

            self.global_start -= samples;
            self.clip_start -= samples;
        } else {
            let samples = samples as u32;
            let samples = min(samples, self.global_end - self.global_start);

            self.global_start += samples;
            self.clip_start += samples;
        }
    }

    pub fn trim_end(&mut self, samples: i32) {
        if samples < 0 {
            let samples = -samples as u32;
            let samples = min(samples, self.global_end - self.global_start);

            self.global_end -= samples;
        } else {
            let samples = samples as u32;
            let samples = min(samples, self.audio.len() - self.clip_start);

            self.global_end += samples;
        }
    }

    pub fn move_by(&mut self, samples: i32) {
        if samples < 0 {
            let samples = -samples as u32;
            let samples = min(samples, self.global_end - self.global_start);

            self.global_start -= samples;
            self.global_end -= samples;
        } else {
            let samples = samples as u32;

            self.global_start += samples;
            self.global_end += samples;
        }
    }
}

pub fn read_audio_file(path: &PathBuf, config: &StreamConfig) -> Result<Arc<InterleavedAudio>> {
    let mut samples = Vec::new();

    let format = symphonia::default::get_probe().format(
        &Hint::new(),
        MediaSourceStream::new(
            Box::new(File::open(path).expect("Can't open file")),
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
    let name = path.file_name().unwrap().to_str().unwrap().to_string();

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
                    let duration = audio_buf.capacity() as u64;
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

    if sample_rate == config.sample_rate.0 {
        return Ok(Arc::new(InterleavedAudio::new(samples.into(), name)));
    }

    let mut resampler = SincFixedIn::<f32>::new(
        config.sample_rate.0 as f64 / sample_rate as f64,
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

    Ok(Arc::new(InterleavedAudio::new(samples.into(), name)))
}
