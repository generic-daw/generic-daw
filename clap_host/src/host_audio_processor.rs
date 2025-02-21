use async_channel::{Receiver, Sender};
use clack_host::prelude::*;

#[derive(Debug)]
pub struct HostAudioProcessor {
    pub sender: Sender<(Vec<Vec<f32>>, EventBuffer)>,
    pub receiver: Receiver<(Vec<Vec<f32>>, EventBuffer)>,
}
