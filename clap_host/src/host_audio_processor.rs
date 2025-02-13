use crate::{HostThreadMessage, MainThreadMessage};
use std::sync::mpsc::{Receiver, Sender};

pub struct HostAudioProcessor {
    pub sender: Sender<MainThreadMessage>,
    pub receiver: Receiver<HostThreadMessage>,
}
