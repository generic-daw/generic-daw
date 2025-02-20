use crate::{PluginId, gui::GuiExt, host::Host, timer::Timers};
use clack_extensions::timer::PluginTimer;
use clack_host::prelude::*;
use dpi::{PhysicalSize, Size};
use raw_window_handle::RawWindowHandle;
use std::{
    cell::RefCell,
    fmt::{Debug, Formatter},
    rc::Rc,
};

pub struct ClapPluginGui {
    pub(crate) instance: PluginInstance<Host>,
    pub(crate) gui: GuiExt,
    pub(crate) id: PluginId,
}

impl Debug for ClapPluginGui {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClapPluginGui").finish_non_exhaustive()
    }
}

impl ClapPluginGui {
    #[must_use]
    pub fn plugin_id(&self) -> PluginId {
        self.id
    }

    pub fn call_on_main_thread_callback(&mut self) {
        self.instance.call_on_main_thread_callback();
    }

    #[must_use]
    pub fn needs_floating(&self) -> Option<bool> {
        self.gui.needs_floating()
    }

    pub fn open_embedded(&mut self, window_handle: RawWindowHandle) {
        self.gui
            .open_embedded(&mut self.instance.plugin_handle(), window_handle);
    }

    pub fn open_floating(&mut self) {
        self.gui.open_floating(&mut self.instance.plugin_handle());
    }

    pub fn destroy(&mut self) {
        self.gui.destroy(&mut self.instance.plugin_handle());
    }

    #[must_use]
    pub fn can_resize(&self) -> bool {
        self.gui.can_resize
    }

    #[must_use]
    pub fn resize(&mut self, width: u32, height: u32) -> [u32; 2] {
        let size = self
            .gui
            .resize(
                &mut self.instance.plugin_handle(),
                Size::Physical(PhysicalSize::new(width, height)),
            )
            .to_logical(1.0);

        [size.width, size.height]
    }

    #[must_use]
    pub fn timers(&self) -> Option<(Rc<RefCell<Timers>>, PluginTimer)> {
        self.instance
            .access_handler(|h| h.timer_support.map(|ext| (h.timers.clone(), ext)))
    }

    #[must_use]
    pub fn plugin_handle(&mut self) -> PluginMainThreadHandle<'_> {
        self.instance.plugin_handle()
    }
}

impl Drop for ClapPluginGui {
    fn drop(&mut self) {
        self.destroy();
    }
}
