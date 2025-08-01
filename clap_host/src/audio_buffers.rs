use crate::audio_ports_config::AudioPortsConfig;
use clack_host::prelude::*;
use generic_daw_utils::{NoDebug, RotateConcatExt as _};

#[derive(Debug)]
pub struct AudioBuffers {
	config: PluginAudioConfiguration,

	input_config: AudioPortsConfig,
	output_config: AudioPortsConfig,

	input_ports: NoDebug<AudioPorts>,
	output_ports: NoDebug<AudioPorts>,

	input_buffers: NoDebug<Box<[Box<[f32]>]>>,
	output_buffers: NoDebug<Box<[Box<[f32]>]>>,

	latency_comp: Vec<f32>,
}

impl AudioBuffers {
	pub fn new(
		config: PluginAudioConfiguration,
		input_config: AudioPortsConfig,
		output_config: AudioPortsConfig,
		latency: u32,
	) -> Self {
		let input_ports = AudioPorts::from(&input_config).into();
		let output_ports = AudioPorts::from(&output_config).into();

		let input_buffers = input_config
			.port_channel_counts
			.iter()
			.map(|c| vec![0.0; config.max_frames_count as usize * c].into_boxed_slice())
			.collect::<Box<[_]>>()
			.into();
		let output_buffers = output_config
			.port_channel_counts
			.iter()
			.map(|c| vec![0.0; config.max_frames_count as usize * c].into_boxed_slice())
			.collect::<Box<[_]>>()
			.into();

		let latency_comp = vec![0.0; latency as usize * 2];

		Self {
			config,

			input_config,
			output_config,

			input_ports,
			output_ports,

			input_buffers,
			output_buffers,

			latency_comp,
		}
	}

	pub fn read_in(&mut self, buf: &[f32]) {
		let input_buffer = &mut self.input_buffers[self.input_config.main_port_index];
		let n_channels = *self
			.input_config
			.port_channel_counts
			.get(self.input_config.main_port_index)
			.unwrap_or(&0);

		if n_channels == 1 {
			buf.chunks_exact(2)
				.map(|c| c.iter().sum())
				.zip(input_buffer)
				.for_each(|(buf, sample)| *sample = buf);
		} else if n_channels != 0 {
			let (l, r) = input_buffer.split_at_mut(self.config.max_frames_count as usize);

			buf.iter()
				.step_by(2)
				.zip(l)
				.for_each(|(buf, sample)| *sample = *buf);

			buf.iter()
				.skip(1)
				.step_by(2)
				.zip(r)
				.for_each(|(buf, sample)| *sample = *buf);
		}
	}

	pub fn prepare(&mut self, frames: usize) -> (InputAudioBuffers<'_>, OutputAudioBuffers<'_>) {
		let input_frames = self
			.input_config
			.port_channel_counts
			.get(self.input_config.main_port_index)
			.filter(|&&x| x != 0)
			.map_or(frames, |n_channels| frames / n_channels);

		let output_frames = self
			.output_config
			.port_channel_counts
			.get(self.output_config.main_port_index)
			.filter(|&&x| x != 0)
			.map_or(frames, |n_channels| frames / n_channels);

		(
			self.input_ports
				.with_input_buffers(self.input_buffers.iter_mut().map(|c| {
					AudioPortBuffer {
						latency: 0,
						channels: AudioPortBufferType::f32_input_only(
							c.chunks_exact_mut(self.config.max_frames_count as usize)
								.map(|b| &mut b[..input_frames])
								.map(InputChannel::variable),
						),
					}
				})),
			self.output_ports
				.with_output_buffers(self.output_buffers.iter_mut().map(|c| {
					AudioPortBuffer {
						latency: 0,
						channels: AudioPortBufferType::f32_output_only(
							c.chunks_exact_mut(self.config.max_frames_count as usize)
								.map(|b| &mut b[..output_frames]),
						),
					}
				})),
		)
	}

	pub fn write_out(&mut self, buf: &mut [f32], mix_level: f32) {
		self.latency_comp.rotate_right_concat(buf);

		let output_buffer = &self.output_buffers[self.output_config.main_port_index];
		let n_channels = *self
			.output_config
			.port_channel_counts
			.get(self.output_config.main_port_index)
			.unwrap_or(&0);

		if n_channels == 1 {
			output_buffer
				.iter()
				.flat_map(|x| [x, x])
				.zip(&mut *buf)
				.for_each(|(sample, buf)| {
					*buf *= 1.0 - mix_level;
					*buf += sample * mix_level;
				});
		} else if n_channels != 0 {
			let (l, r) = output_buffer.split_at(self.config.max_frames_count as usize);

			l.iter()
				.zip(r)
				.flat_map(<[&f32; 2]>::from)
				.zip(&mut *buf)
				.for_each(|(sample, buf)| {
					*buf *= 1.0 - mix_level;
					*buf += sample * mix_level;
				});
		}
	}

	pub fn latency_changed(&mut self, latency: u32) {
		self.latency_comp.resize(latency as usize * 2, 0.0);
	}

	pub fn delay(&self) -> usize {
		self.latency_comp.len()
	}
}
