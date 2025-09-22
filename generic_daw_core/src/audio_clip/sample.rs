use crate::{LOD_LEVELS, lod::create_lods, resampler::Resampler};
use generic_daw_utils::NoDebug;
use log::info;
use std::{fs::File, path::Path, sync::Arc};
use symphonia::core::{
	audio::SampleBuffer,
	codecs::DecoderOptions,
	formats::FormatOptions,
	io::{MediaSourceStream, MediaSourceStreamOptions},
	meta::MetadataOptions,
	probe::Hint,
};

#[derive(Debug)]
pub struct Sample {
	pub samples: NoDebug<Box<[f32]>>,
	pub lods: NoDebug<Box<[Box<[(f32, f32)]>; LOD_LEVELS]>>,
	pub path: Arc<Path>,
	pub name: Arc<str>,
}

impl Sample {
	#[must_use]
	pub fn create(path: Arc<Path>, sample_rate: u32) -> Option<Arc<Self>> {
		info!("loading sample {}", path.display());

		let name = path.file_name()?.to_str()?.into();
		let samples = Self::read_audio_file(&path, sample_rate)?;
		let lods = create_lods(&samples);

		info!("loaded sample {}", path.display());

		Some(Arc::new(Self {
			samples: samples.into(),
			lods: lods.into(),
			path,
			name,
		}))
	}

	fn read_audio_file(path: impl AsRef<Path>, sample_rate: u32) -> Option<Box<[f32]>> {
		let mut format = symphonia::default::get_probe()
			.format(
				&Hint::default(),
				MediaSourceStream::new(
					Box::new(File::open(path).ok()?),
					MediaSourceStreamOptions::default(),
				),
				&FormatOptions::default(),
				&MetadataOptions::default(),
			)
			.ok()?
			.format;

		let track = format.default_track()?;
		let track_id = track.id;
		let n_channels = track.codec_params.channels?.count();
		let n_frames = track.codec_params.n_frames? as usize;
		let file_sample_rate = track.codec_params.sample_rate?;
		let delay = track.codec_params.delay.unwrap_or_default() as usize;
		let padding = track.codec_params.padding.unwrap_or_default() as usize;

		let mut stereo = Vec::with_capacity(2 * n_frames);

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
		}

		let stereo = &stereo[2 * delay..stereo.len() - 2 * padding];

		if file_sample_rate == sample_rate {
			Some(stereo.into())
		} else {
			let mut resampler = Resampler::new(file_sample_rate as usize, sample_rate as usize)?;
			resampler.process(stereo);
			Some(resampler.finish().into_boxed_slice())
		}
	}
}
