use crate::{AudioBuffer, Host, audio_ports_config::AudioPortsConfig};
use clack_host::{prelude::*, process::StartedPluginAudioProcessor};
use std::fmt::{Debug, Formatter};

pub struct AudioProcessor {
    started_processor: StartedPluginAudioProcessor<Host>,
    steady_time: u64,

    pub config: PluginAudioConfiguration,

    pub input_config: AudioPortsConfig,
    pub output_config: AudioPortsConfig,

    pub input_ports: AudioPorts,
    pub output_ports: AudioPorts,

    pub input_channels: AudioBuffer,
    pub output_channels: AudioBuffer,
}

impl Debug for AudioProcessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginAudioProcessor")
            .field("config", &self.config)
            .field("steady_time", &self.steady_time)
            .finish_non_exhaustive()
    }
}

impl AudioProcessor {
    pub(crate) fn new(
        started_processor: StartedPluginAudioProcessor<Host>,
        config: PluginAudioConfiguration,
        input_config: AudioPortsConfig,
        output_config: AudioPortsConfig,
    ) -> Self {
        let input_ports = (&input_config).into();
        let output_ports = (&output_config).into();

        let input_channels = input_config
            .port_channel_counts
            .iter()
            .map(|c| vec![0.0; config.max_frames_count as usize * c])
            .collect();
        let output_channels = output_config
            .port_channel_counts
            .iter()
            .map(|c| vec![0.0; config.max_frames_count as usize * c])
            .collect();

        Self {
            started_processor,
            steady_time: 0,

            config,

            input_config,
            output_config,

            input_ports,
            output_ports,

            input_channels,
            output_channels,
        }
    }

    pub fn process(
        &mut self,
        buf: &mut [f32],
        input_events: &InputEvents<'_>,
        output_events: &mut OutputEvents<'_>,
    ) {
        let channels = self.output_config.port_channel_counts[self.output_config.main_port_index];
        let frames = buf.len() / channels;

        let input_audio = self
            .input_ports
            .with_input_buffers(self.input_channels.iter_mut().map(|c| {
                AudioPortBuffer {
                    latency: 0,
                    channels: AudioPortBufferType::f32_input_only(
                        c.chunks_exact_mut(self.config.max_frames_count as usize)
                            .map(|b| &mut b[..frames])
                            .map(InputChannel::constant),
                    ),
                }
            }));

        let mut output_audio =
            self.output_ports
                .with_output_buffers(self.output_channels.iter_mut().map(|c| {
                    AudioPortBuffer {
                        latency: 0,
                        channels: AudioPortBufferType::f32_output_only(
                            c.chunks_exact_mut(self.config.max_frames_count as usize)
                                .map(|b| &mut b[..frames]),
                        ),
                    }
                }));

        self.started_processor
            .process(
                &input_audio,
                &mut output_audio,
                input_events,
                output_events,
                Some(self.steady_time),
                None,
            )
            .unwrap();

        self.steady_time += u64::from(output_audio.frames_count().unwrap());

        self.output_channels[self.output_config.main_port_index]
            .chunks_exact(self.config.max_frames_count as usize)
            .enumerate()
            .for_each(|(i, c)| {
                c.iter()
                    .zip(buf.iter_mut().skip(i).step_by(channels))
                    .for_each(|(sample, buf)| *buf = *sample);
            });
    }
}
