use anyhow::{anyhow, Result};
use rubato::{
    SincFixedIn, SincInterpolationParameters, SincInterpolationType, VecResampler, WindowFunction,
};
use std::{
    cmp::{max_by, min_by},
    fs::File,
    path::PathBuf,
    sync::{atomic::Ordering::SeqCst, mpsc::Sender, Arc, RwLock},
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    errors::Error,
    formats::FormatOptions,
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
    probe::Hint,
};

use crate::{
    generic_back::position::{Meter, Position},
    generic_front::timeline::Message,
};

use super::TrackClip;
type Wave = Vec<(f32, f32)>;
pub struct InterleavedAudio {
    samples: Arc<[RwLock<Wave>]>,
}

impl InterleavedAudio {
    pub fn new(samples: &[f32]) -> Self {
        let length = samples.len();
        Self {
            samples: Arc::new([
                RwLock::new(samples.iter().map(|s| (*s, *s)).collect()),
                RwLock::new(vec![(0.0, 0.0); (length + 1) / 2]),
                RwLock::new(vec![(0.0, 0.0); (length + 3) / 4]),
                RwLock::new(vec![(0.0, 0.0); (length + 7) / 8]),
                RwLock::new(vec![(0.0, 0.0); (length + 15) / 16]),
                RwLock::new(vec![(0.0, 0.0); (length + 31) / 32]),
                RwLock::new(vec![(0.0, 0.0); (length + 63) / 64]),
                RwLock::new(vec![(0.0, 0.0); (length + 127) / 128]),
                RwLock::new(vec![(0.0, 0.0); (length + 255) / 256]),
                RwLock::new(vec![(0.0, 0.0); (length + 511) / 512]),
                RwLock::new(vec![(0.0, 0.0); (length + 1023) / 1024]),
                RwLock::new(vec![(0.0, 0.0); (length + 2047) / 2048]),
                RwLock::new(vec![(0.0, 0.0); (length + 4095) / 4096]),
                RwLock::new(vec![(0.0, 0.0); (length + 8191) / 8192]),
            ]),
        }
    }

    pub fn len(&self) -> u32 {
        u32::try_from(self.samples[0].read().unwrap().len()).unwrap()
    }

    pub fn get_sample_at_index(&self, ver: usize, index: usize) -> (f32, f32) {
        *self.samples[ver]
            .read()
            .unwrap()
            .get(index)
            .unwrap_or(&(0.0, 0.0))
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

    pub fn get_ver_at_index(&self, ver: usize, index: usize) -> (f32, f32) {
        let (min, max) = self.audio.get_sample_at_index(ver, index);
        (min * self.volume, max * self.volume)
    }
}

impl TrackClip for AudioClip {
    fn get_at_global_time(&self, global_time: u32, meter: &Meter) -> f32 {
        if !meter.playing.load(SeqCst) {
            return 0.0;
        }
        self.get_ver_at_index(
            0,
            (global_time - (self.global_start + self.clip_start).in_interleaved_samples(meter))
                as usize,
        )
        .0
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

pub fn read_audio_file(
    path: &PathBuf,
    meter: &Meter,
    sender: Sender<Message>,
) -> Result<Arc<InterleavedAudio>> {
    let mut samples = Vec::<f32>::new();

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
        let interleaved_audio = Arc::new(InterleavedAudio::new(&samples));
        return Ok(create_downscaled_audio(interleaved_audio, sender));
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

    let interleaved_audio = Arc::new(InterleavedAudio::new(&samples));
    Ok(create_downscaled_audio(interleaved_audio, sender))
}

fn create_downscaled_audio(
    audio: Arc<InterleavedAudio>,
    sender: Sender<Message>,
) -> Arc<InterleavedAudio> {
    let audio_clone = audio.clone();
    std::thread::spawn(move || {
        (1..audio.samples.len()).for_each(|i| {
            let len = audio.samples[i].read().unwrap().len();
            let last = audio.samples[i - 1].read().unwrap();
            (0..len).for_each(|j| {
                audio.samples[i].write().unwrap()[j] = (
                    min_by(
                        last[2 * j].0,
                        last.get(2 * j + 1).unwrap_or(&(f32::MAX, f32::MAX)).0,
                        |a, b| a.partial_cmp(b).unwrap(),
                    ),
                    max_by(
                        last[2 * j].1,
                        last.get(2 * j + 1).unwrap_or(&(-f32::MAX, -f32::MAX)).1,
                        |a, b| a.partial_cmp(b).unwrap(),
                    ),
                );
            });
            sender.send(Message::ArrangementUpdated).unwrap();
        });
    });
    audio_clone
}
