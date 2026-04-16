use crate::{MediaSource, Transport};
use std::{num::NonZero, sync::Arc};
use symphonia::core::{
	audio::SampleBuffer,
	codecs::DecoderOptions,
	formats::FormatOptions,
	io::{MediaSourceStream, MediaSourceStreamOptions},
	meta::MetadataOptions,
	probe::Hint,
};
use utils::{NoDebug, unique_id};

unique_id!(sample_id);

pub use sample_id::Id as SampleId;

#[derive(Debug)]
pub struct Sample {
	pub id: SampleId,
	pub samples: NoDebug<Arc<[f32]>>,
	#[expect(clippy::struct_field_names)]
	pub sample_rate: NonZero<u32>,
}

impl Sample {
	#[must_use]
	pub fn new(source: Box<dyn MediaSource>) -> Option<Self> {
		let mut format = symphonia::default::get_probe()
			.format(
				&Hint::default(),
				MediaSourceStream::new(source, MediaSourceStreamOptions::default()),
				&FormatOptions::default(),
				&MetadataOptions::default(),
			)
			.ok()?
			.format;

		let track = format.default_track()?;
		let track_id = track.id;
		let sample_rate = NonZero::new(track.codec_params.sample_rate?)?;
		let n_frames = track.codec_params.n_frames.unwrap_or_default() as usize;
		let n_channels = track.codec_params.channels?.count();
		let mut delay = track.codec_params.delay.unwrap_or_default() as usize;
		let padding = track.codec_params.padding.unwrap_or_default() as usize;

		let mut samples = Vec::with_capacity(2 * n_frames);

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

			if n_channels == 1 {
				samples.extend(
					sample_buf
						.samples()
						.iter()
						.skip(2 * delay)
						.flat_map(|x| [x, x]),
				);
			} else if n_channels != 0 {
				samples.extend(
					sample_buf
						.samples()
						.chunks_exact(n_channels)
						.skip(2 * delay)
						.flat_map(|x| [x[0], x[1]]),
				);
			}

			delay = delay.saturating_sub(audio_buf.frames());
		}

		samples.truncate(samples.len().saturating_sub(2 * padding));

		Some(Self {
			id: SampleId::unique(),
			samples: NoDebug(samples.into()),
			sample_rate,
		})
	}

	#[must_use]
	pub fn resample_ratio(&self, transport: &Transport) -> f32 {
		self.sample_rate.get() as f32 / transport.sample_rate.get() as f32
	}
}

pub fn resample_cubic(audio: &mut [f32], samples: &[f32], resample_ratio: f32, offset: usize) {
	for (frame, [l, r]) in audio.as_chunks_mut().0.iter_mut().enumerate() {
		let frame = (offset + frame) as f32 * resample_ratio;
		let fract = frame.fract();
		let idx = 2 * frame as usize;

		let mut done = true;

		let [l3, r3] = if idx + 6 > samples.len() {
			[0.0, 0.0]
		} else {
			done = false;
			[samples[idx + 4], samples[idx + 5]]
		};

		let [l2, r2] = if idx + 4 > samples.len() {
			[0.0, 0.0]
		} else {
			done = false;
			[samples[idx + 2], samples[idx + 3]]
		};

		let [l1, r1] = if idx + 2 > samples.len() {
			[0.0, 0.0]
		} else {
			done = false;
			[samples[idx], samples[idx + 1]]
		};

		let [l0, r0] = if idx < 2 || idx > samples.len() {
			[0.0, 0.0]
		} else {
			done = false;
			[samples[idx - 2], samples[idx - 1]]
		};

		if done {
			break;
		}

		*l += interp_cubic(l0, l1, l2, l3, fract);
		*r += interp_cubic(r0, r1, r2, r3, fract);
	}
}

fn interp_cubic(s0: f32, s1: f32, s2: f32, s3: f32, fract: f32) -> f32 {
	let c0 = s1;
	let c1 = 0.5 * (s2 - s0);
	let c2 = s0 - 2.5 * s1 + 2.0 * s2 - 0.5 * s3;
	let c3 = 0.5 * (s3 - s0) + 1.5 * (s1 - s2);
	((c3 * fract + c2) * fract + c1) * fract + c0
}
