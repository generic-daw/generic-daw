use crate::{MainThreadMessage, audio_processor::AudioThreadMessage};
use async_channel::Sender;
use clack_extensions::{
    gui::{GuiSize, HostGuiImpl},
    log::{HostLogImpl, LogSeverity},
};
use clack_host::prelude::*;
use tracing::{debug, error, info, warn};

#[derive(Debug)]
pub struct Shared {
    pub main_sender: Sender<MainThreadMessage>,
    pub audio_sender: Sender<AudioThreadMessage>,
}

impl SharedHandler<'_> for Shared {
    fn request_process(&self) {}

    fn request_callback(&self) {
        self.main_sender
            .try_send(MainThreadMessage::RequestCallback)
            .unwrap();
    }

    fn request_restart(&self) {
        self.audio_sender
            .try_send(AudioThreadMessage::RequestRestart)
            .unwrap();
    }
}

impl HostGuiImpl for Shared {
    fn resize_hints_changed(&self) {}

    fn request_resize(&self, new_size: GuiSize) -> Result<(), HostError> {
        Ok(self
            .main_sender
            .try_send(MainThreadMessage::GuiRequestResize(new_size))?)
    }

    fn request_show(&self) -> Result<(), HostError> {
        Ok(self
            .main_sender
            .try_send(MainThreadMessage::GuiRequestShow)?)
    }

    fn request_hide(&self) -> Result<(), HostError> {
        Ok(self
            .main_sender
            .try_send(MainThreadMessage::GuiRequestHide)?)
    }

    fn closed(&self, _was_destroyed: bool) {
        self.main_sender
            .try_send(MainThreadMessage::GuiClosed)
            .unwrap();
    }
}

impl HostLogImpl for Shared {
    fn log(&self, severity: LogSeverity, message: &str) {
        match severity {
            LogSeverity::Info => info!(message),
            LogSeverity::Debug => debug!(message),
            LogSeverity::Warning => warn!(message),
            LogSeverity::Error
            | LogSeverity::Fatal
            | LogSeverity::PluginMisbehaving
            | LogSeverity::HostMisbehaving => error!(message),
        }
    }
}

impl Shared {
    pub fn new(
        main_sender: Sender<MainThreadMessage>,
        audio_sender: Sender<AudioThreadMessage>,
    ) -> Self {
        Self {
            main_sender,
            audio_sender,
        }
    }
}
