use generic_daw_utils::NoDebug;
use rubato::{FftFixedIn, Resampler as _};

#[derive(Debug)]
pub struct Resampler {
	resample_ratio: f64,

	fft: NoDebug<FftFixedIn<f32>>,
	input_buffer: Vec<Vec<f32>>,
	output_buffer: Vec<Vec<f32>>,
	output: Vec<f32>,

	frames_written: usize,
	start_trim_left: usize,
	end_trim_left: usize,
}

impl Resampler {
	pub fn new(sample_rate_input: usize, sample_rate_output: usize) -> Option<Self> {
		Self::with_capacity(sample_rate_input, sample_rate_output, 0)
	}

	pub fn with_capacity(
		sample_rate_input: usize,
		sample_rate_output: usize,
		frames: usize,
	) -> Option<Self> {
		let fft = FftFixedIn::new(sample_rate_input, sample_rate_output, 1024, 1, 2).ok()?;
		let resample_ratio = sample_rate_output as f64 / sample_rate_input as f64;
		let input_buffer = fft.input_buffer_allocate(false);
		let output_buffer = fft.output_buffer_allocate(true);
		let start_trim_left = fft.output_delay();

		Some(Self {
			resample_ratio,

			fft: fft.into(),
			input_buffer,
			output_buffer,
			output: Vec::with_capacity(2 * frames),

			frames_written: 0,
			start_trim_left,
			end_trim_left: 0,
		})
	}

	pub fn trim_start(mut self, frames: usize) -> Self {
		self.start_trim_left =
			self.fft.output_delay() + (frames as f64 * self.resample_ratio) as usize;
		self
	}

	pub fn trim_end(mut self, frames: usize) -> Self {
		self.end_trim_left = (frames as f64 * self.resample_ratio) as usize;
		self
	}

	pub fn process(&mut self, mut samples: &[f32]) {
		let mut len;

		while {
			len = (self.fft.input_frames_next() - self.input_buffer[0].len())
				* self.input_buffer.len();
			samples.len() > len
		} {
			self.process_inner(&samples[..len]);
			samples = &samples[len..];
		}

		self.process_inner(samples);
	}

	fn process_inner(&mut self, samples: &[f32]) {
		let channels = self.input_buffer.len();
		for (i, buf) in self.input_buffer.iter_mut().enumerate() {
			buf.extend(samples.iter().skip(i).step_by(channels).copied());
		}

		if self.input_buffer[0].len() >= self.fft.input_frames_next() {
			let (frames_in, frames_out) = self
				.fft
				.process_into_buffer(&self.input_buffer, &mut self.output_buffer, None)
				.unwrap();

			for i in self.start_trim_left..frames_out {
				for buf in &self.output_buffer {
					self.output.push(buf[i]);
				}
			}

			for buf in &mut self.input_buffer {
				buf.drain(0..frames_in);
			}

			self.frames_written += frames_in;
			self.start_trim_left = self.start_trim_left.saturating_sub(frames_out);
		}
	}

	pub fn finish(mut self) -> Vec<f32> {
		let input_frames = self.frames_written + self.input_buffer[0].len();
		let expected_output_frames =
			(input_frames as f64 * self.resample_ratio).ceil() as usize - self.end_trim_left;

		while self.frames_written < expected_output_frames {
			let len = self.fft.input_frames_next();

			for buf in &mut self.input_buffer {
				buf.resize(len, 0.0);
			}

			self.process_inner(&[]);
		}

		self.output
			.truncate(expected_output_frames * self.input_buffer.len());

		self.output
	}

	pub fn samples(&self) -> &[f32] {
		&self.output
	}
}
