use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler as _};
use std::num::NonZero;
use utils::{NoDebug, boxed_slice};

#[derive(Debug)]
pub struct Resampler {
	fft: NoDebug<Fft<f32>>,

	input_buffer: Vec<f32>,
	output_buffer: Box<[f32]>,
	output: Vec<f32>,

	frames_in: usize,
	frames_out: usize,

	trim_start: usize,
	trim_end: usize,
}

impl Resampler {
	pub fn new(
		sample_rate_input: NonZero<u32>,
		sample_rate_output: NonZero<u32>,
		nbr_channels: NonZero<usize>,
	) -> Option<Self> {
		let fft = Fft::new(
			sample_rate_input.get() as usize,
			sample_rate_output.get() as usize,
			1024,
			2,
			nbr_channels.get(),
			FixedSync::Both,
		)
		.ok()?;
		let input_buffer = Vec::with_capacity(fft.input_frames_max() * fft.nbr_channels());
		let output_buffer = boxed_slice![0.0; fft.output_frames_max() * fft.nbr_channels()];

		Some(Self {
			fft: fft.into(),

			input_buffer,
			output_buffer,
			output: Vec::new(),

			frames_in: 0,
			frames_out: 0,

			trim_start: 0,
			trim_end: 0,
		})
	}

	pub fn trim_start(mut self, frames: usize) -> Self {
		self.trim_start = frames;
		self
	}

	pub fn trim_end(mut self, frames: usize) -> Self {
		self.trim_end = frames;
		self
	}

	pub fn reserve(mut self, frames: usize) -> Self {
		let channels = self.fft.nbr_channels();
		let resample_ratio = self.fft.resample_ratio();
		let frames =
			((frames - self.trim_start - self.trim_end) as f64 * resample_ratio).ceil() as usize;
		self.output
			.reserve_exact(channels * (frames + self.fft.output_frames_max()));
		self
	}

	pub fn process(&mut self, mut samples: &[f32]) {
		let channels = self.fft.nbr_channels();
		debug_assert!(samples.len().is_multiple_of(channels));

		if channels * self.trim_start > samples.len() {
			self.trim_start -= samples.len() / channels;
			return;
		}

		samples = &samples[channels * self.trim_start..];
		self.trim_start = 0;

		let mut len;
		while {
			let frames = self.fft.input_frames_next();
			len = frames * channels - self.input_buffer.len();
			len <= samples.len()
		} {
			self.process_inner(&samples[..len]);
			samples = &samples[len..];
		}

		self.process_inner(samples);
	}

	fn process_inner(&mut self, samples: &[f32]) {
		let channels = self.fft.nbr_channels();
		let input_frames = self.fft.input_frames_next();
		let output_frames = self.fft.output_frames_next();
		debug_assert!(samples.len().is_multiple_of(channels));

		self.input_buffer.extend_from_slice(samples);

		if self.input_buffer.len() >= input_frames * channels {
			let input_buffer =
				InterleavedSlice::new(&self.input_buffer, channels, input_frames).unwrap();

			let mut output_buffer =
				InterleavedSlice::new_mut(&mut self.output_buffer, channels, output_frames)
					.unwrap();

			let (frames_in, frames_out) = self
				.fft
				.process_into_buffer(&input_buffer, &mut output_buffer, None)
				.unwrap();

			self.input_buffer.drain(0..frames_in * channels);

			let frames_delay = self.fft.output_delay().saturating_sub(self.frames_out);
			self.output.extend_from_slice(
				&self.output_buffer[frames_delay * channels..frames_out * channels],
			);

			self.frames_in += frames_in;
			self.frames_out += frames_out;
		}
	}

	pub fn finish(mut self) -> Vec<f32> {
		let channels = self.fft.nbr_channels();
		let resample_ratio = self.fft.resample_ratio();
		let frames_in = self.frames_in + self.input_buffer.len() / channels - self.trim_end;
		let frames_out = (frames_in as f64 * resample_ratio).ceil() as usize;

		while self.output.len() < channels * frames_out {
			let frames = self.fft.input_frames_next();
			self.input_buffer.resize(frames * channels, 0.0);
			self.process_inner(&[]);
		}

		self.output.truncate(channels * frames_out);

		self.output
	}

	pub fn samples(&self) -> &[f32] {
		&self.output
	}
}
