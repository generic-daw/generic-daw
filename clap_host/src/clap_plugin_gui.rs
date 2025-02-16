use super::{gui::GuiExt, host::Host};
use clack_host::prelude::*;
use winit::dpi::Size;

pub struct ClapPluginGui {
    instance: PluginInstance<Host>,
    gui: GuiExt,
}

impl ClapPluginGui {
    pub(crate) fn new(instance: PluginInstance<Host>, gui: GuiExt) -> Self {
        Self { instance, gui }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.gui.resize(
            &mut self.instance.plugin_handle(),
            Size::Physical(winit::dpi::PhysicalSize::new(width, height)),
        );
    }

    pub fn call_on_main_thread_callback(&mut self) {
        self.instance.call_on_main_thread_callback();
    }
}

impl Drop for ClapPluginGui {
    fn drop(&mut self) {
        self.gui.destroy(&mut self.instance.plugin_handle());
    }
}
