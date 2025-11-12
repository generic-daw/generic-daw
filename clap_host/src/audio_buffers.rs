use crate::{audio_ports_config::AudioPortsConfig, host::Host};
use clack_host::prelude::*;
use generic_daw_utils::{AudioRingbuf, NoDebug};

#[derive(Debug)]
pub struct AudioBuffers {
	config: PluginAudioConfiguration,

	input_config: AudioPortsConfig,
	output_config: AudioPortsConfig,

	input_ports: NoDebug<AudioPorts>,
	output_ports: NoDebug<AudioPorts>,

	input_buffers: NoDebug<Box<[Box<[f32]>]>>,
	output_buffers: NoDebug<Box<[Box<[f32]>]>>,

	latency_comp: AudioRingbuf,
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
			.map(|c| vec![0.0; config.max_frames_count as usize * c].into_boxed_slice())
			.collect::<Box<_>>()
			.into();
		let output_buffers = output_config
			.port_channel_counts
			.iter()
			.map(|c| vec![0.0; config.max_frames_count as usize * c].into_boxed_slice())
			.collect::<Box<_>>()
			.into();

		let latency_comp = AudioRingbuf::default();

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
		let Some(input_buffer) = self
			.input_buffers
			.get_mut(self.input_config.main_port_index)
		else {
			return;
		};

		let n_channels = *self
			.input_config
			.port_channel_counts
			.get(self.input_config.main_port_index)
			.unwrap_or(&0);

		let buf = buf.as_chunks().0.iter();

		if n_channels == 1 {
			buf.map(|[l, r]| l + r)
				.zip(input_buffer)
				.for_each(|(buf, sample)| *sample = buf);
		} else if n_channels != 0 {
			let (l, r) = input_buffer.split_at_mut(self.config.max_frames_count as usize);

			buf.zip(l.iter_mut().zip(r)).for_each(|(buf, (l, r))| {
				*l = buf[0];
				*r = buf[1];
			});
		}
	}

	pub fn prepare(&mut self, frames: usize) -> (InputAudioBuffers<'_>, OutputAudioBuffers<'_>) {
		(
			self.input_ports
				.with_input_buffers(self.input_buffers.iter_mut().map(|c| {
					AudioPortBuffer {
						latency: 0,
						channels: AudioPortBufferType::f32_input_only(
							c.chunks_exact_mut(self.config.max_frames_count as usize)
								.map(|b| &mut b[..frames])
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
								.map(|b| &mut b[..frames]),
						),
					}
				})),
		)
	}

	pub fn write_out(&mut self, buf: &mut [f32], mix_level: f32) {
		self.latency_comp.shift(buf);

		let Some(output_buffer) = self.output_buffers.get(self.output_config.main_port_index)
		else {
			return;
		};

		let n_channels = *self
			.output_config
			.port_channel_counts
			.get(self.output_config.main_port_index)
			.unwrap_or(&0);

		let buf = buf.as_chunks_mut::<2>().0.iter_mut();

		if n_channels == 1 {
			output_buffer.iter().zip(buf).for_each(|(sample, buf)| {
				buf[0] = buf[0].mul_add(1.0 - mix_level, sample * mix_level);
				buf[1] = buf[1].mul_add(1.0 - mix_level, sample * mix_level);
			});
		} else if n_channels != 0 {
			let (l, r) = output_buffer.split_at(self.config.max_frames_count as usize);

			l.iter().zip(r).zip(buf).for_each(|((l, r), buf)| {
				buf[0] = buf[0].mul_add(1.0 - mix_level, l * mix_level);
				buf[1] = buf[1].mul_add(1.0 - mix_level, r * mix_level);
			});
		}
	}

	pub fn latency_changed(&mut self, latency: u32) {
		self.latency_comp.resize(2 * latency as usize);
	}

	pub fn delay(&self) -> usize {
		self.latency_comp.len()
	}
}
