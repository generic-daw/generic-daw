use crate::{HostThreadMessage, MainThreadMessage};
use async_channel::{Receiver, Sender};
use std::fmt::{Debug, Formatter};

pub struct HostAudioProcessor {
    pub sender: Sender<MainThreadMessage>,
    pub receiver: Receiver<HostThreadMessage>,
}

impl Debug for HostAudioProcessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostAudioProcessor").finish_non_exhaustive()
    }
}
