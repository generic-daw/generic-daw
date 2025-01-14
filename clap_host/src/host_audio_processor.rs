use crate::{HostThreadMessage, MainThreadMessage};
use std::sync::mpsc::{Receiver, Sender};

#[derive(Debug)]
pub struct HostAudioProcessor {
    pub sender: Sender<MainThreadMessage>,
    pub receiver: Receiver<HostThreadMessage>,
}
