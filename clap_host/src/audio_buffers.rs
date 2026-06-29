use crate::{audio_ports_config::AudioPortsConfig, host::Host};
use clack_host::prelude::*;
use dsp::DelayLine;
use utils::{NoDebug, boxed_slice};

#[derive(Debug)]
pub struct AudioBuffers {
	config: PluginAudioConfiguration,

	input_config: AudioPortsConfig,
	output_config: AudioPortsConfig,

	input_ports: NoDebug<AudioPorts>,
	output_ports: NoDebug<AudioPorts>,

	input_buffers: NoDebug<Box<[Box<[f32]>]>>,
	output_buffers: NoDebug<Box<[Box<[f32]>]>>,

	steady_time: u64,
	delay_line: DelayLine,
}

impl AudioBuffers {
	pub fn new(plugin: &mut PluginInstance<Host>, config: PluginAudioConfiguration) -> Self {
		let input_config = AudioPortsConfig::from_ports(plugin, true).unwrap_or_default();
		let output_config = AudioPortsConfig::from_ports(plugin, false).unwrap_or_default();

		let input_ports = AudioPorts::from(&input_config).into();
		let output_ports = AudioPorts::from(&output_config).into();

		let input_buffers = input_config
			.port_channel_counts
			.iter()
			.map(|c| boxed_slice![0.0; (config.max_frames_count * c) as usize])
			.collect::<Box<_>>()
			.into();
		let output_buffers = output_config
			.port_channel_counts
			.iter()
			.map(|c| boxed_slice![0.0; (config.max_frames_count * c) as usize])
			.collect::<Box<_>>()
			.into();

		Self {
			config,

			input_config,
			output_config,

			input_ports,
			output_ports,

			input_buffers,
			output_buffers,

			steady_time: 0,
			delay_line: DelayLine::default(),
		}
	}

	pub fn read_in(&mut self, buf: &[[f32; 2]]) -> u64 {
		if let Some(input_buffer) = self
			.input_buffers
			.get_mut(self.input_config.main_port_index)
		{
			let n_channels = *self
				.input_config
				.port_channel_counts
				.get(self.input_config.main_port_index)
				.unwrap_or(&0);

			if n_channels == 1 {
				buf.iter()
					.zip(input_buffer)
					.for_each(|(buf, sample)| *sample = (buf[0] + buf[1]) / 2.0);
			} else if n_channels != 0 {
				let (l, r) = input_buffer.split_at_mut(self.config.max_frames_count as usize);

				buf.iter()
					.zip(l.iter_mut().zip(r))
					.for_each(|(buf, (l, r))| {
						*l = buf[0];
						*r = buf[1];
					});
			}
		}

		let steady_time = self.steady_time;
		self.steady_time += buf.len() as u64;
		steady_time
	}

	pub fn prepare(&mut self, len: usize) -> (InputAudioBuffers<'_>, OutputAudioBuffers<'_>) {
		let input_audio = self
			.input_ports
			.with_input_buffers(self.input_buffers.iter_mut().map(|c| {
				AudioPortBuffer {
					latency: 0,
					channels: AudioPortBufferType::f32_input_only(
						c.chunks_exact_mut(self.config.max_frames_count as usize)
							.map(|b| &mut b[..len])
							.map(InputChannel::variable),
					),
				}
			}));

		let output_audio =
			self.output_ports
				.with_output_buffers(self.output_buffers.iter_mut().map(|c| {
					AudioPortBuffer {
						latency: 0,
						channels: AudioPortBufferType::f32_output_only(
							c.chunks_exact_mut(self.config.max_frames_count as usize)
								.map(|b| &mut b[..len]),
						),
					}
				}));

		(input_audio, output_audio)
	}

	pub fn are_inputs_quiet(&self) -> bool {
		!self
			.input_buffers
			.iter()
			.flatten()
			.any(|f| f.abs() >= f32::EPSILON)
	}

	pub fn are_outputs_quiet(&self) -> bool {
		!self
			.output_buffers
			.iter()
			.flatten()
			.any(|f| f.abs() >= f32::EPSILON)
	}

	pub fn flush(&mut self, buf: &mut [[f32; 2]], mix_level: f32) {
		self.delay_line.advance(buf);

		for sample in buf.as_flattened_mut() {
			*sample *= 1.0 - mix_level;
		}
	}

	pub fn write_out(&mut self, buf: &mut [[f32; 2]], mix_level: f32) {
		self.delay_line.advance(buf);

		let Some(output_buffer) = self.output_buffers.get(self.output_config.main_port_index)
		else {
			return;
		};

		let n_channels = *self
			.output_config
			.port_channel_counts
			.get(self.output_config.main_port_index)
			.unwrap_or(&0);

		let buf_mix = 1f32.copysign(mix_level) - mix_level;
		let sample_mix = mix_level.abs();

		if n_channels == 1 {
			output_buffer.iter().zip(buf).for_each(|(sample, buf)| {
				buf[0] = buf[0] * buf_mix + sample * sample_mix;
				buf[1] = buf[1] * buf_mix + sample * sample_mix;
			});
		} else if n_channels != 0 {
			let (l, r) = output_buffer.split_at(self.config.max_frames_count as usize);

			l.iter().zip(r).zip(buf).for_each(|((l, r), buf)| {
				buf[0] = buf[0] * buf_mix + l * sample_mix;
				buf[1] = buf[1] * buf_mix + r * sample_mix;
			});
		}
	}

	pub fn latency(&self) -> usize {
		self.delay_line.len()
	}

	pub fn set_latency(&mut self, latency: u32) {
		self.delay_line.resize(latency as usize);
	}

	pub fn reset(&mut self) {
		self.steady_time = 0;
		self.delay_line.reset();
	}
}
