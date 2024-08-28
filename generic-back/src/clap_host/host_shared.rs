use clack_extensions::gui::{GuiSize, HostGuiImpl, PluginGui};
use clack_host::{
    host::{HostError, MainThreadHandler, SharedHandler},
    plugin::InitializedPluginHandle,
    prelude::{AudioPorts, EventBuffer},
};
use std::sync::{mpsc::Sender, Arc, Mutex};

pub enum PluginThreadMessage {
    RunOnMainThread,
    GuiClosed,
    GuiRequestResized(GuiSize),
    ProcessAudio(
        [[f32; 8]; 2],
        Arc<Mutex<AudioPorts>>,
        Arc<Mutex<AudioPorts>>,
        EventBuffer,
        EventBuffer,
    ),
    GetCounter,
}

pub enum HostThreadMessage {
    AudioProcessed([[f32; 8]; 2], EventBuffer),
    Counter(u32),
}

pub struct HostShared {
    sender: Sender<PluginThreadMessage>,
}

impl<'a> SharedHandler<'a> for HostShared {
    fn request_process(&self) {}
    fn request_callback(&self) {}
    fn request_restart(&self) {}
}

impl HostGuiImpl for HostShared {
    fn resize_hints_changed(&self) {}

    fn request_resize(&self, new_size: GuiSize) -> Result<(), HostError> {
        Ok(self
            .sender
            .send(PluginThreadMessage::GuiRequestResized(new_size))?)
    }

    fn request_show(&self) -> Result<(), HostError> {
        Ok(())
    }

    fn request_hide(&self) -> Result<(), HostError> {
        Ok(())
    }

    fn closed(&self, _was_destroyed: bool) {
        self.sender.send(PluginThreadMessage::GuiClosed).unwrap();
    }
}

impl HostShared {
    pub const fn new(sender: Sender<PluginThreadMessage>) -> Self {
        Self { sender }
    }
}

pub struct HostPluginThread<'a> {
    plugin: Option<InitializedPluginHandle<'a>>,
    pub gui: Option<PluginGui>,
}

impl<'a> MainThreadHandler<'a> for HostPluginThread<'a> {
    fn initialized(&mut self, instance: InitializedPluginHandle<'a>) {
        self.gui = instance.get_extension();
        self.plugin = Some(instance);
    }
}

impl<'a> HostPluginThread<'a> {
    pub const fn new() -> Self {
        Self {
            plugin: None,
            gui: None,
        }
    }
}

impl<'a> Default for HostPluginThread<'a> {
    fn default() -> Self {
        Self::new()
    }
}
