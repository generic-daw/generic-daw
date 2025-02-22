use crate::{AudioBuffer, Host, audio_ports_config::AudioPortsConfig};
use async_channel::{Receiver, Sender};
use clack_host::{prelude::*, process::StartedPluginAudioProcessor};
use std::fmt::{Debug, Formatter};

pub struct PluginAudioProcessor {
    started_processor: StartedPluginAudioProcessor<Host>,
    config: PluginAudioConfiguration,
    steady_time: u64,

    pub input_ports: AudioPorts,
    pub output_ports: AudioPorts,

    pub input_channels: AudioBuffer,
    pub output_channels: AudioBuffer,

    pub sender: Sender<(AudioBuffer, EventBuffer)>,
    pub receiver: Receiver<(AudioBuffer, EventBuffer)>,
}

impl Debug for PluginAudioProcessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginAudioProcessor")
            .field("config", &self.config)
            .field("steady_time", &self.steady_time)
            .field("sender", &self.sender)
            .field("receiver", &self.receiver)
            .finish_non_exhaustive()
    }
}

impl PluginAudioProcessor {
    pub(crate) fn new(
        started_processor: StartedPluginAudioProcessor<Host>,
        config: PluginAudioConfiguration,
        input_config: &AudioPortsConfig,
        output_config: &AudioPortsConfig,
        sender: Sender<(AudioBuffer, EventBuffer)>,
        receiver: Receiver<(AudioBuffer, EventBuffer)>,
    ) -> Self {
        let input_ports = input_config.into();
        let output_ports = output_config.into();

        let input_channels = input_config
            .port_channel_counts
            .iter()
            .map(|c| vec![0.0; config.max_frames_count as usize * c].into_boxed_slice())
            .collect();
        let output_channels = output_config
            .port_channel_counts
            .iter()
            .map(|c| vec![0.0; config.max_frames_count as usize * c].into_boxed_slice())
            .collect();

        Self {
            started_processor,
            config,
            steady_time: 0,
            input_ports,
            output_ports,
            input_channels,
            output_channels,
            sender,
            receiver,
        }
    }

    pub fn process(
        &mut self,
        input_events: &InputEvents<'_>,
        output_events: &mut OutputEvents<'_>,
    ) {
        let input_audio = self
            .input_ports
            .with_input_buffers(self.input_channels.iter_mut().map(|c| {
                AudioPortBuffer {
                    latency: 0,
                    channels: AudioPortBufferType::f32_input_only(
                        c.chunks_exact_mut(self.config.max_frames_count as usize)
                            .map(InputChannel::constant),
                    ),
                }
            }));

        let mut output_audio =
            self.output_ports
                .with_output_buffers(self.output_channels.iter_mut().map(|c| AudioPortBuffer {
                    latency: 0,
                    channels: AudioPortBufferType::f32_output_only(
                        c.chunks_exact_mut(self.config.max_frames_count as usize),
                    ),
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
    }
}
