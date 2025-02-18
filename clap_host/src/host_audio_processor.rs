use crate::{HostThreadMessage, MainThreadMessage};
use async_channel::{Receiver, Sender};

pub struct HostAudioProcessor {
    pub sender: Sender<MainThreadMessage>,
    pub receiver: Receiver<HostThreadMessage>,
}
