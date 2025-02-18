use crate::MainThreadMessage;
use async_channel::Sender;
use clack_extensions::{
    gui::{GuiSize, HostGuiImpl},
    params::HostParamsImplShared,
};
use clack_host::prelude::*;

pub struct Shared {
    sender: Sender<MainThreadMessage>,
}

impl SharedHandler<'_> for Shared {
    fn request_process(&self) {}

    fn request_callback(&self) {
        self.sender
            .try_send(MainThreadMessage::RequestCallback)
            .unwrap();
    }

    fn request_restart(&self) {}

    fn initializing(&self, _: InitializingPluginHandle<'_>) {}
}

impl HostGuiImpl for Shared {
    fn resize_hints_changed(&self) {}

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
    fn request_flush(&self) {}
}

impl Shared {
    pub fn new(sender: Sender<MainThreadMessage>) -> Self {
        Self { sender }
    }
}
