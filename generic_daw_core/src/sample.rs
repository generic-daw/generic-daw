use crate::resampler::Resampler;
use generic_daw_utils::{NoDebug, unique_id};
use symphonia::core::{
	audio::SampleBuffer,
	codecs::DecoderOptions,
	formats::FormatOptions,
	io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions},
	meta::MetadataOptions,
	probe::Hint,
};

unique_id!(sample_id);

pub use sample_id::Id as SampleId;

#[derive(Debug)]
pub struct Sample {
	pub id: SampleId,
	pub samples: NoDebug<Box<[f32]>>,
}

impl Sample {
	#[must_use]
	pub fn new(source: Box<dyn MediaSource>, sample_rate: u32) -> Option<Self> {
		Some(Self {
			samples: Self::read_audio_file(source, sample_rate)?.into(),
			id: SampleId::unique(),
		})
	}

	fn read_audio_file(source: Box<dyn MediaSource>, sample_rate: u32) -> Option<Box<[f32]>> {
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
		let n_channels = track.codec_params.channels?.count();
		let delay = track.codec_params.delay.unwrap_or_default() as usize;
		let padding = track.codec_params.padding.unwrap_or_default() as usize;

		let mut resampler = Resampler::new(
			track.codec_params.sample_rate? as usize,
			sample_rate as usize,
			2,
		)?
		.trim_start(delay)
		.trim_end(padding)
		.reserve(track.codec_params.n_frames.unwrap_or_default() as usize);

		let mut stereo = Vec::with_capacity(
			2 * track.codec_params.max_frames_per_packet.unwrap_or_default() as usize,
		);

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

			if n_channels == 2 {
				stereo.extend(sample_buf.samples());
			} else if n_channels == 1 {
				stereo.extend(sample_buf.samples().iter().flat_map(|x| [x, x]));
			} else if n_channels != 0 {
				stereo.extend(
					sample_buf
						.samples()
						.chunks_exact(n_channels)
						.flat_map(|x| [x[0], x[1]]),
				);
			}

			resampler.process(&stereo);

			stereo.clear();
		}

		Some(resampler.finish().into_boxed_slice())
	}
}
