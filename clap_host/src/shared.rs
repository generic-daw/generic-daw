use crate::GuiMessage;
use async_channel::Sender;
use clack_extensions::{
    gui::{GuiSize, HostGuiImpl},
    log::{HostLogImpl, LogSeverity},
};
use clack_host::prelude::*;
use tracing::{debug, error, info, warn};

pub struct Shared {
    pub sender: Sender<GuiMessage>,
}

impl SharedHandler<'_> for Shared {
    fn request_process(&self) {}

    fn request_callback(&self) {
        self.sender.try_send(GuiMessage::RequestCallback).unwrap();
    }

    fn request_restart(&self) {}

    fn initializing(&self, _instance: InitializingPluginHandle<'_>) {}
}

impl HostGuiImpl for Shared {
    fn resize_hints_changed(&self) {}

    fn request_resize(&self, new_size: GuiSize) -> Result<(), HostError> {
        Ok(self
            .sender
            .try_send(GuiMessage::GuiRequestResize(new_size))?)
    }

    fn request_show(&self) -> Result<(), HostError> {
        Ok(self.sender.try_send(GuiMessage::GuiRequestShow)?)
    }

    fn request_hide(&self) -> Result<(), HostError> {
        Ok(self.sender.try_send(GuiMessage::GuiRequestHide)?)
    }

    fn closed(&self, _was_destroyed: bool) {
        self.sender.try_send(GuiMessage::GuiClosed).unwrap();
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
    pub fn new(sender: Sender<GuiMessage>) -> Self {
        Self { sender }
    }
}
