use crate::audio_ports_config::AudioPortsConfig;
use clack_host::{prelude::*, process::PluginAudioConfiguration};
use generic_daw_utils::NoDebug;

#[derive(Debug)]
pub struct AudioBuffers {
    config: PluginAudioConfiguration,

    input_config: AudioPortsConfig,
    output_config: AudioPortsConfig,

    input_ports: NoDebug<AudioPorts>,
    output_ports: NoDebug<AudioPorts>,

    input_channels: NoDebug<Box<[Box<[f32]>]>>,
    output_channels: NoDebug<Box<[Box<[f32]>]>>,
}

impl AudioBuffers {
    pub fn new(
        config: PluginAudioConfiguration,
        input_config: AudioPortsConfig,
        output_config: AudioPortsConfig,
    ) -> Self {
        let input_ports = AudioPorts::from(&input_config).into();
        let output_ports = AudioPorts::from(&output_config).into();

        let input_channels = input_config
            .port_channel_counts
            .iter()
            .map(|c| vec![0.0; config.max_frames_count as usize * c].into_boxed_slice())
            .collect::<Box<[_]>>()
            .into();
        let output_channels = output_config
            .port_channel_counts
            .iter()
            .map(|c| vec![0.0; config.max_frames_count as usize * c].into_boxed_slice())
            .collect::<Box<[_]>>()
            .into();

        Self {
            config,

            input_config,
            output_config,

            input_ports,
            output_ports,

            input_channels,
            output_channels,
        }
    }

    pub fn read_in(&mut self, buf: &[f32]) {
        if self
            .input_config
            .port_channel_counts
            .get(self.input_config.main_port_index)
            .unwrap_or(&0)
            == &0
        {
        } else if self.input_config.port_channel_counts[self.input_config.main_port_index] == 1 {
            buf.iter()
                .flat_map(|x| [x, x])
                .zip(&mut *self.input_channels[self.input_config.main_port_index])
                .for_each(|(buf, sample)| *sample = *buf);
        } else {
            let (l, r) = self.input_channels[self.input_config.main_port_index]
                .split_at_mut(self.config.max_frames_count as usize);

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

    pub fn prepare(&mut self, buf: &[f32]) -> (InputAudioBuffers<'_>, OutputAudioBuffers<'_>) {
        let input_frames = buf.len() / 2;

        let output_channels =
            self.output_config.port_channel_counts[self.output_config.main_port_index].clamp(1, 2);
        let output_frames = buf.len() / output_channels;

        (
            self.input_ports
                .with_input_buffers(self.input_channels.iter_mut().map(|c| {
                    AudioPortBuffer {
                        latency: 0,
                        channels: AudioPortBufferType::f32_input_only(
                            c.chunks_exact_mut(self.config.max_frames_count as usize)
                                .map(|b| &mut b[..input_frames])
                                .map(InputChannel::constant),
                        ),
                    }
                })),
            self.output_ports
                .with_output_buffers(self.output_channels.iter_mut().map(|c| {
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

    pub fn write_out(&self, buf: &mut [f32], mix_level: f32) {
        if self
            .output_config
            .port_channel_counts
            .get(self.output_config.main_port_index)
            .unwrap_or(&0)
            == &0
        {
        } else if self.output_config.port_channel_counts[self.output_config.main_port_index] == 1 {
            self.output_channels[self.output_config.main_port_index]
                .iter()
                .flat_map(|x| [x, x])
                .zip(&mut *buf)
                .for_each(|(sample, buf)| {
                    *buf *= 1.0 - mix_level;
                    *buf += sample * mix_level;
                });
        } else {
            let (l, r) = self.output_channels[self.output_config.main_port_index]
                .split_at(self.config.max_frames_count as usize);

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
}
