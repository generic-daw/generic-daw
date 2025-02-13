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

    pub fn resize(&mut self, width: f32, height: f32) {
        self.gui.resize(
            &mut self.instance.plugin_handle(),
            Size::Physical(winit::dpi::PhysicalSize::new(width as u32, height as u32)),
        );
    }

    pub fn destroy(mut self) {
        self.gui.destroy(&mut self.instance.plugin_handle());
    }
}
