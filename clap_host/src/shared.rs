use crate::{MainThreadMessage, PluginDescriptor, audio_processor::AudioThreadMessage};
use async_channel::Sender;
use clack_extensions::{
    gui::{GuiSize, HostGuiImpl},
    log::{HostLogImpl, LogSeverity},
};
use clack_host::prelude::*;
use log::{debug, error, info, warn};

#[derive(Debug)]
pub struct Shared {
    pub descriptor: PluginDescriptor,
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
        self.main_sender
            .try_send(MainThreadMessage::GuiRequestResize(new_size))
            .unwrap();

        Ok(())
    }

    fn request_show(&self) -> Result<(), HostError> {
        self.main_sender
            .try_send(MainThreadMessage::GuiRequestShow)
            .unwrap();

        Ok(())
    }

    fn request_hide(&self) -> Result<(), HostError> {
        self.main_sender
            .try_send(MainThreadMessage::GuiRequestHide)
            .unwrap();

        Ok(())
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
            LogSeverity::Debug => {
                debug!("{}: {message}", self.descriptor);
            }
            LogSeverity::Info => {
                info!("{}: {message}", self.descriptor);
            }
            LogSeverity::Warning => {
                warn!("{}: {message}", self.descriptor);
            }
            LogSeverity::Error
            | LogSeverity::Fatal
            | LogSeverity::HostMisbehaving
            | LogSeverity::PluginMisbehaving => {
                error!("{}: {message}", self.descriptor);
            }
        }
    }
}

impl Shared {
    pub fn new(
        descriptor: PluginDescriptor,
        main_sender: Sender<MainThreadMessage>,
        audio_sender: Sender<AudioThreadMessage>,
    ) -> Self {
        Self {
            descriptor,
            main_sender,
            audio_sender,
        }
    }
}
