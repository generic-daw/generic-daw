use super::{gui::GuiExt, host::Host};
use clack_host::prelude::*;
use std::fmt::Debug;
use winit::dpi::Size;

pub struct ClapPluginGui {
    instance: PluginInstance<Host>,
    gui: GuiExt,
}

impl Debug for ClapPluginGui {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClapPlugin").finish_non_exhaustive()
    }
}

impl ClapPluginGui {
    #[must_use]
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
