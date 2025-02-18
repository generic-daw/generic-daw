use crate::{gui::GuiExt, host::Host, timer::Timers};
use clack_extensions::timer::PluginTimer;
use clack_host::prelude::*;
use std::{fmt::Debug, rc::Rc};
use winit::{dpi::Size, raw_window_handle::RawWindowHandle};

pub struct ClapPluginGui {
    pub(crate) instance: PluginInstance<Host>,
    pub(crate) gui: GuiExt,
}

impl Debug for ClapPluginGui {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClapPluginGui").finish_non_exhaustive()
    }
}

impl ClapPluginGui {
    pub fn resize(&mut self, width: u32, height: u32) {
        self.gui.resize(
            &mut self.instance.plugin_handle(),
            Size::Physical(winit::dpi::PhysicalSize::new(width, height)),
        );
    }

    pub fn call_on_main_thread_callback(&mut self) {
        self.instance.call_on_main_thread_callback();
    }

    #[must_use]
    pub fn needs_floating(&self) -> Option<bool> {
        self.gui.needs_floating()
    }

    pub fn destroy(&mut self) {
        self.gui.destroy(&mut self.instance.plugin_handle());
    }

    pub fn open_embedded(&mut self, window_handle: RawWindowHandle) {
        self.gui
            .open_embedded(&mut self.instance.plugin_handle(), window_handle);
    }

    pub fn open_floating(&mut self) {
        self.gui.open_floating(&mut self.instance.plugin_handle());
    }

    #[must_use]
    pub fn timers(&self) -> Option<(Rc<Timers>, PluginTimer)> {
        self.instance
            .access_handler(|h| h.timer_support.map(|ext| (h.timers.clone(), ext)))
    }
}

impl Drop for ClapPluginGui {
    fn drop(&mut self) {
        self.destroy();
    }
}
