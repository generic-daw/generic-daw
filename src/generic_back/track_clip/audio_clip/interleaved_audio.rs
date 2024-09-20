use crate::{
    generic_back::{arrangement::Arrangement, gcd},
    generic_front::timeline::Message,
};
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
    sync::{atomic::Ordering::SeqCst, mpsc::Sender, Arc, RwLock},
};
use symphonia::core::{
    audio::SampleBuffer,
    errors::Error,
    formats::FormatOptions,
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
    probe::Hint,
};

type Lod = Vec<(f32, f32)>;
pub struct InterleavedAudio {
    /// these are used to play the sample back
    samples: Vec<f32>,
    /// these are used to draw the sample in various quality levels
    lods: [RwLock<Lod>; 10],
    /// the file name associated with the sample
    pub name: String,
}

impl InterleavedAudio {
    pub fn create(
        path: &PathBuf,
        arrangement: &Arc<Arrangement>,
        sender: Sender<Message>,
    ) -> Result<Arc<Self>> {
        let mut samples = Self::read_audio_file(path, arrangement)?;
        samples.shrink_to_fit();

        let length = samples.len();
        let audio = Arc::new(Self {
            samples,
            lods: [
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
            ],
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
        });

        Self::create_lod(audio.clone(), sender);
        Ok(audio)
    }

    pub(super) fn len(&self) -> u32 {
        u32::try_from(self.samples.len()).unwrap()
    }

    pub(super) fn get_sample_at_index(&self, index: u32) -> f32 {
        self.samples[usize::try_from(index).unwrap()]
    }

    pub fn get_lod_at_index(&self, lod: u32, index: u32) -> (f32, f32) {
        *self.lods[usize::try_from(lod).unwrap()]
            .read()
            .unwrap()
            .get(usize::try_from(index).unwrap())
            .unwrap_or(&(0.0, 0.0))
    }

    fn read_audio_file(path: &PathBuf, arrangement: &Arc<Arrangement>) -> Result<Vec<f32>> {
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
        let file_sample_rate = track.codec_params.sample_rate.unwrap();

        let mut decoder = symphonia::default::get_codecs()
            .make(
                &track.codec_params,
                &symphonia::core::codecs::DecoderOptions::default(),
            )
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

        let stream_sample_rate = arrangement.meter.sample_rate.load(SeqCst);

        if file_sample_rate == stream_sample_rate {
            return Ok(samples);
        }

        let resample_ratio = f64::from(stream_sample_rate) / f64::from(file_sample_rate);

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

        Ok(samples)
    }

    fn create_lod(audio: Arc<Self>, sender: Sender<Message>) {
        std::thread::spawn(move || {
            audio.samples.chunks(8).enumerate().for_each(|(i, chunk)| {
                let (min, max) = match chunk.iter().minmax_by(|a, b| a.partial_cmp(b).unwrap()) {
                    MinMax(min, max) => (min, max),
                    OneElement(x) => (x, x),
                    NoElements => unreachable!(),
                };
                audio.lods[0].write().unwrap()[i] =
                    ((*min).mul_add(0.5, 0.5), (*max).mul_add(0.5, 0.5));
            });
            sender.send(Message::ArrangementUpdated).unwrap();

            (1..audio.lods.len()).for_each(|i| {
                let len = audio.lods[i].read().unwrap().len();
                let last = audio.lods[i - 1].read().unwrap();
                (0..len).for_each(|j| {
                    audio.lods[i].write().unwrap()[j] = (
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
    }
}
