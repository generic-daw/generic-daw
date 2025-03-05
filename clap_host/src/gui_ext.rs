use crate::{PluginId, host::Host, timer_ext::TimerExt};
use clack_extensions::{
    gui::{GuiApiType, GuiConfiguration, GuiSize, PluginGui, Window as ClapWindow},
    timer::PluginTimer,
};
use clack_host::prelude::*;
use raw_window_handle::RawWindowHandle;
use std::{
    cell::RefCell,
    fmt::{Debug, Formatter},
    rc::Rc,
};

pub struct GuiExt {
    instance: PluginInstance<Host>,
    plugin_gui: PluginGui,
    name: Box<str>,
    id: PluginId,
    is_floating: bool,
    is_open: bool,
    can_resize: bool,
}

impl Debug for GuiExt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiExt")
            .field("id", &self.id)
            .field("is_floating", &self.is_floating)
            .field("is_open", &self.is_open)
            .field("can_resize", &self.can_resize)
            .finish_non_exhaustive()
    }
}

impl GuiExt {
    #[must_use]
    pub fn new(plugin_gui: PluginGui, mut instance: PluginInstance<Host>, name: Box<str>) -> Self {
        let mut config = GuiConfiguration {
            api_type: GuiApiType::default_for_current_platform().unwrap(),
            is_floating: false,
        };

        let mut plugin = instance.plugin_handle();

        let config = if plugin_gui.is_api_supported(&mut plugin, config) {
            config
        } else {
            config.is_floating = true;
            if plugin_gui.is_api_supported(&mut plugin, config) {
                config
            } else {
                panic!()
            }
        };

        Self {
            instance,
            plugin_gui,
            name,
            id: PluginId::unique(),
            is_floating: config.is_floating,
            is_open: false,
            can_resize: false,
        }
    }

    #[must_use]
    pub fn plugin_id(&self) -> PluginId {
        self.id
    }

    #[must_use]
    pub fn get_size(&mut self) -> Option<[u32; 2]> {
        self.plugin_gui
            .get_size(&mut self.instance.plugin_handle())
            .map(|size| [size.width, size.height])
    }

    pub fn call_on_main_thread_callback(&mut self) {
        self.instance.call_on_main_thread_callback();
    }

    pub fn plugin_handle(&mut self) -> PluginMainThreadHandle<'_> {
        self.instance.plugin_handle()
    }

    #[must_use]
    pub fn timers(&self) -> Option<(Rc<RefCell<TimerExt>>, PluginTimer)> {
        self.instance
            .access_handler(|h| h.timer_support.map(|ext| (h.timers.clone(), ext)))
    }

    #[must_use]
    pub fn is_floating(&self) -> bool {
        self.is_floating
    }

    #[must_use]
    pub fn can_resize(&self) -> bool {
        self.can_resize
    }

    pub fn open_floating(&mut self) {
        assert!(self.is_floating);

        self.begin_open();

        self.finish_open();
    }

    pub fn open_embedded(&mut self, window_handle: RawWindowHandle) {
        assert!(!self.is_floating);

        self.begin_open();

        // SAFETY:
        // We destroy the plugin ui just before the window is closed
        unsafe {
            self.plugin_gui.set_parent(
                &mut self.instance.plugin_handle(),
                ClapWindow::from_window_handle(window_handle).unwrap(),
            )
        }
        .unwrap();

        self.finish_open();
    }

    fn begin_open(&mut self) {
        let config = GuiConfiguration {
            api_type: GuiApiType::default_for_current_platform().unwrap(),
            is_floating: self.is_floating(),
        };

        self.plugin_gui
            .create(&mut self.instance.plugin_handle(), config)
            .unwrap();
    }

    fn finish_open(&mut self) {
        let mut plugin = self.instance.plugin_handle();

        self.plugin_gui.show(&mut plugin).unwrap();
        self.can_resize = self.plugin_gui.can_resize(&mut plugin);
        self.is_open = true;
    }

    #[must_use]
    pub fn resize(&mut self, width: u32, height: u32) -> [u32; 2] {
        let mut plugin = self.instance.plugin_handle();
        let size = GuiSize { width, height };

        if self.can_resize {
            let size = self
                .plugin_gui
                .adjust_size(&mut plugin, size)
                .unwrap_or(size);
            self.plugin_gui.set_size(&mut plugin, size).unwrap();
        };

        let size = self.plugin_gui.get_size(&mut plugin).unwrap_or(size);
        [size.width, size.height]
    }

    pub fn destroy(&mut self) {
        if self.is_open {
            self.plugin_gui.destroy(&mut self.instance.plugin_handle());
            self.is_open = false;
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for GuiExt {
    fn drop(&mut self) {
        self.destroy();
    }
}
