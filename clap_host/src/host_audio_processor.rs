use crate::{AudioBuffer, audio_ports_config::AudioPortsConfig};
use async_channel::{Receiver, Sender};
use clack_host::prelude::*;

#[derive(Debug)]
pub struct HostAudioProcessor {
    pub sender: Sender<(AudioBuffer, EventBuffer)>,
    pub receiver: Receiver<(AudioBuffer, EventBuffer)>,

    pub input_config: AudioPortsConfig,
    pub output_config: AudioPortsConfig,
}
