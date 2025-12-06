use rubato::{FftFixedIn, Resampler as _};
use std::num::NonZero;
use utils::NoDebug;

#[derive(Debug)]
pub struct Resampler {
	fft: NoDebug<FftFixedIn<f32>>,
	resample_ratio: f64,

	input_buffer: Box<[Vec<f32>]>,
	output_buffer: Box<[Box<[f32]>]>,
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
		let fft = FftFixedIn::new(
			sample_rate_input.get() as usize,
			sample_rate_output.get() as usize,
			1024,
			2,
			nbr_channels.get(),
		)
		.ok()?;
		let resample_ratio =
			f64::from(sample_rate_output.get()) / f64::from(sample_rate_input.get());
		let input_buffer = fft.input_buffer_allocate(false).into_boxed_slice();
		let output_buffer = fft
			.output_buffer_allocate(true)
			.into_iter()
			.map(Vec::into_boxed_slice)
			.collect();

		Some(Self {
			fft: fft.into(),
			resample_ratio,

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
		let channels = self.input_buffer.len();
		let frames = ((frames - self.trim_start - self.trim_end) as f64 * self.resample_ratio)
			.ceil() as usize;
		self.output
			.reserve_exact(channels * frames + self.fft.output_frames_max());
		self
	}

	pub fn process(&mut self, mut samples: &[f32]) {
		let channels = self.input_buffer.len();
		debug_assert!(samples.len().is_multiple_of(channels));

		if channels * self.trim_start > samples.len() {
			self.trim_start -= samples.len() / channels;
			return;
		}

		samples = &samples[channels * self.trim_start..];
		self.trim_start = 0;

		let mut len;
		while {
			len = channels * (self.fft.input_frames_next() - self.input_buffer[0].len());
			samples.len() > len
		} {
			self.process_inner(&samples[..len]);
			samples = &samples[len..];
		}

		self.process_inner(samples);
	}

	fn process_inner(&mut self, samples: &[f32]) {
		let channels = self.input_buffer.len();
		debug_assert!(samples.len().is_multiple_of(channels));

		for (i, buf) in self.input_buffer.iter_mut().enumerate() {
			buf.extend(samples.iter().skip(i).step_by(channels).copied());
		}

		if self.input_buffer[0].len() >= self.fft.input_frames_next() {
			let (frames_in, frames_out) = self
				.fft
				.process_into_buffer(&self.input_buffer, &mut self.output_buffer, None)
				.unwrap();

			for i in self.fft.output_delay().saturating_sub(self.frames_out)..frames_out {
				for buf in &self.output_buffer {
					self.output.push(buf[i]);
				}
			}

			for buf in &mut self.input_buffer {
				buf.drain(0..frames_in);
			}

			self.frames_in += frames_in;
			self.frames_out += frames_out;
		}
	}

	pub fn finish(mut self) -> Vec<f32> {
		let channels = self.input_buffer.len();
		let frames_in = self.frames_in + self.input_buffer[0].len() - self.trim_end;
		let frames_out = (frames_in as f64 * self.resample_ratio).ceil() as usize;

		while self.output.len() < channels * frames_out {
			let len = self.fft.input_frames_next();

			for buf in &mut self.input_buffer {
				buf.resize(len, 0.0);
			}

			self.process_inner(&[]);
		}

		self.output.truncate(channels * frames_out);

		self.output
	}

	pub fn samples(&self) -> &[f32] {
		&self.output
	}
}
