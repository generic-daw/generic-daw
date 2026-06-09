use crate::{MediaSource, Transport, time::SecondsTime};
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

#[derive(Clone, Debug)]
pub struct Sample {
	pub id: SampleId,
	pub samples: NoDebug<Arc<[[f32; 2]]>>,
	#[expect(clippy::struct_field_names)]
	pub sample_rate: NonZero<u32>,
}

impl Sample {
	#[must_use]
	pub fn new(source: Box<dyn MediaSource>) -> Option<Self> {
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
		let num_frames = track.num_frames.unwrap_or_default() as usize;
		let mut delay = track.delay.unwrap_or_default() as usize;
		let padding = track.padding.unwrap_or_default() as usize;
		let codec_params = track.codec_params.as_ref()?.audio()?;
		let sample_rate = NonZero::new(codec_params.sample_rate?)?;
		let channels = codec_params.channels.as_ref()?.count();
		let max_frames_per_packet = codec_params.max_frames_per_packet.unwrap_or_default() as usize;

		let mut decoder = symphonia::default::get_codecs()
			.make_audio_decoder(codec_params, &AudioDecoderOptions::default())
			.ok()?;

		let mut samples = Vec::with_capacity(num_frames);
		let mut packet_buf = Vec::with_capacity(channels * max_frames_per_packet);

		while let Some(packet) = format.next_packet().ok()? {
			if packet.track_id != track_id {
				continue;
			}

			decoder
				.decode(&packet)
				.ok()?
				.copy_to_vec_interleaved(&mut packet_buf);

			if channels == 1 {
				samples.extend(packet_buf.iter().skip(delay).map(|&x| [x, x]));
			} else if channels != 0 {
				samples.extend(
					packet_buf
						.chunks_exact(channels)
						.skip(delay)
						.map(|x| [x[0], x[1]]),
				);
			}

			delay = delay.saturating_sub(packet_buf.len() / channels);
		}

		samples.truncate(samples.len().saturating_sub(padding));

		Some(Self {
			id: SampleId::unique(),
			samples: NoDebug(samples.into()),
			sample_rate,
		})
	}

	#[must_use]
	pub fn len(&self, transport: &Transport) -> SecondsTime {
		SecondsTime::from_frames(self.samples.len(), transport) * self.resample_ratio(transport)
	}

	#[must_use]
	pub fn resample_ratio(&self, transport: &Transport) -> f64 {
		f64::from(transport.sample_rate.get()) / f64::from(self.sample_rate.get())
	}
}
