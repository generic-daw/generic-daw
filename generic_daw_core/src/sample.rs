use crate::{MediaSource, resampler::Resampler};
use std::{num::NonZero, sync::Arc};
use symphonia::core::{
	codecs::audio::AudioDecoderOptions,
	formats::{FormatOptions, TrackType, probe::Hint},
	io::{MediaSourceStream, MediaSourceStreamOptions},
	meta::MetadataOptions,
};
use utils::{NoDebug, unique_id};

unique_id!(sample_id);

pub use sample_id::Id as SampleId;

#[derive(Debug)]
pub struct Sample {
	pub id: SampleId,
	pub samples: NoDebug<Arc<[f32]>>,
}

impl Sample {
	#[must_use]
	pub fn new(source: Box<dyn MediaSource>, sample_rate: NonZero<u32>) -> Option<Self> {
		let mut format = symphonia::default::get_probe()
			.probe(
				&Hint::default(),
				MediaSourceStream::new(source, MediaSourceStreamOptions::default()),
				FormatOptions::default(),
				MetadataOptions::default(),
			)
			.ok()?;

		let track = format.default_track(TrackType::Audio)?;
		let track_id = track.id;
		let delay = track.delay.unwrap_or_default() as usize;
		let padding = track.padding.unwrap_or_default() as usize;

		let codec_params = track.codec_params.as_ref()?.audio()?;
		let n_channels = codec_params.channels.as_ref()?.count();
		let max_frames_per_packet = codec_params.max_frames_per_packet.unwrap_or_default() as usize;

		let mut resampler = Resampler::new(
			NonZero::new(codec_params.sample_rate?)?,
			sample_rate,
			NonZero::new(2).unwrap(),
		)?
		.trim_start(delay)
		.trim_end(padding)
		.reserve(track.num_frames.unwrap_or_default() as usize);

		let mut sample_buf = Vec::with_capacity(n_channels * max_frames_per_packet);
		let mut stereo = Vec::with_capacity(2 * max_frames_per_packet);

		let mut decoder = symphonia::default::get_codecs()
			.make_audio_decoder(codec_params, &AudioDecoderOptions::default())
			.ok()?;

		while let Some(packet) = format.next_packet().ok()? {
			if packet.track_id() != track_id {
				continue;
			}

			decoder
				.decode(&packet)
				.ok()?
				.copy_to_vec_interleaved(&mut sample_buf);

			if n_channels == 2 {
				stereo.extend(sample_buf.iter());
			} else if n_channels == 1 {
				stereo.extend(sample_buf.iter().flat_map(|x| [x, x]));
			} else if n_channels != 0 {
				stereo.extend(
					sample_buf
						.chunks_exact(n_channels)
						.flat_map(|x| [x[0], x[1]]),
				);
			}

			resampler.process(&stereo);
			stereo.clear();
		}

		Some(Self {
			id: SampleId::unique(),
			samples: NoDebug(resampler.finish().into()),
		})
	}
}
