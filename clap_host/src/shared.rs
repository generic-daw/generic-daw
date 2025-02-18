use crate::MainThreadMessage;
use async_channel::Sender;
use clack_extensions::{
    gui::{GuiSize, HostGuiImpl},
    params::HostParamsImplShared,
    state::PluginState,
};
use clack_host::prelude::*;
use std::sync::OnceLock;

pub struct Shared {
    sender: Sender<MainThreadMessage>,
    pub state: OnceLock<Option<PluginState>>,
}

impl SharedHandler<'_> for Shared {
    fn request_process(&self) {
        // we never pause
    }

    fn request_callback(&self) {
        self.sender
            .try_send(MainThreadMessage::RequestCallback)
            .unwrap();
    }

    fn request_restart(&self) {
        // we don't support restarting plugins (yet)
    }

    fn initializing(&self, instance: InitializingPluginHandle<'_>) {
        self.state.set(instance.get_extension()).ok().unwrap();
    }
}

impl HostGuiImpl for Shared {
    fn resize_hints_changed(&self) {
        // we don't support resize hints (yet)
    }

    fn request_resize(&self, new_size: GuiSize) -> Result<(), HostError> {
        Ok(self
            .sender
            .try_send(MainThreadMessage::GuiRequestResize(new_size))?)
    }

    fn request_show(&self) -> Result<(), HostError> {
        Ok(self.sender.try_send(MainThreadMessage::GuiRequestShow)?)
    }

    fn request_hide(&self) -> Result<(), HostError> {
        Ok(self.sender.try_send(MainThreadMessage::GuiRequestHide)?)
    }

    fn closed(&self, _was_destroyed: bool) {
        self.sender.try_send(MainThreadMessage::GuiClosed).unwrap();
    }
}

impl HostParamsImplShared for Shared {
    fn request_flush(&self) {
        // Can never flush events when not processing: we're never not processing
    }
}

impl Shared {
    pub fn new(sender: Sender<MainThreadMessage>) -> Self {
        Self {
            sender,
            state: OnceLock::new(),
        }
    }
}
