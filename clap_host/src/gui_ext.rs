use crate::{PluginId, host::Host};
use clack_extensions::gui::{
    GuiApiType, GuiConfiguration, GuiSize, PluginGui, Window as ClapWindow,
};
use clack_host::prelude::*;
use generic_daw_utils::NoDebug;
use raw_window_handle::RawWindowHandle;
use std::time::Duration;

#[derive(Debug)]
pub struct GuiExt {
    ext: NoDebug<PluginGui>,
    instance: NoDebug<PluginInstance<Host>>,
    name: Box<str>,
    id: PluginId,
    is_floating: bool,
    is_open: bool,
    can_resize: Option<bool>,
}

impl GuiExt {
    #[must_use]
    pub fn new(
        ext: PluginGui,
        mut instance: PluginInstance<Host>,
        name: Box<str>,
        id: PluginId,
    ) -> Self {
        let mut config = GuiConfiguration {
            api_type: GuiApiType::default_for_current_platform().unwrap(),
            is_floating: false,
        };

        let plugin = &mut instance.plugin_handle();

        if !ext.is_api_supported(plugin, config) {
            config.is_floating = true;

            assert!(ext.is_api_supported(plugin, config));
        }

        Self {
            ext: ext.into(),
            instance: instance.into(),
            name,
            id,
            is_floating: config.is_floating,
            is_open: false,
            can_resize: None,
        }
    }

    #[must_use]
    pub fn plugin_id(&self) -> PluginId {
        self.id
    }

    pub fn call_on_main_thread_callback(&mut self) {
        self.instance.call_on_main_thread_callback();
    }

    #[must_use]
    pub fn tick_timers(&mut self) -> Option<Duration> {
        let timers = self.instance.access_handler(|mt| mt.timers.clone())?;
        timers
            .borrow_mut()
            .tick_timers(&mut self.instance.plugin_handle())
    }

    #[must_use]
    pub fn is_floating(&self) -> bool {
        self.is_floating
    }

    #[must_use]
    pub fn can_resize(&mut self) -> bool {
        *self
            .can_resize
            .get_or_insert_with(|| self.ext.can_resize(&mut self.instance.plugin_handle()))
    }

    #[must_use]
    pub fn get_size(&mut self) -> Option<[u32; 2]> {
        self.ext
            .get_size(&mut self.instance.plugin_handle())
            .map(|size| [size.width, size.height])
    }

    pub fn open_floating(&mut self) {
        assert!(self.is_floating);

        self.open(|_, _| ());
    }

    pub fn open_embedded(&mut self, window_handle: RawWindowHandle) {
        assert!(!self.is_floating);

        self.open(move |ext, plugin| {
            // SAFETY:
            // We destroy the plugin ui just before the window is closed
            unsafe {
                ext.set_parent(
                    plugin,
                    ClapWindow::from_window_handle(window_handle).unwrap(),
                )
            }
            .unwrap();
        });
    }

    fn open(&mut self, f: impl Fn(&PluginGui, &mut PluginMainThreadHandle<'_>)) {
        self.destroy();

        let plugin = &mut self.instance.plugin_handle();

        let config = GuiConfiguration {
            api_type: GuiApiType::default_for_current_platform().unwrap(),
            is_floating: self.is_floating,
        };
        self.ext.create(plugin, config).unwrap();

        f(&self.ext, plugin);

        self.ext.show(plugin).unwrap();
        self.is_open = true;
    }

    #[must_use]
    pub fn resize(&mut self, width: u32, height: u32) -> Option<[u32; 2]> {
        if !self.can_resize.unwrap() {
            return None;
        }

        let mut plugin = self.instance.plugin_handle();
        let size = GuiSize { width, height };

        let size = self.ext.adjust_size(&mut plugin, size).unwrap_or(size);
        self.ext.set_size(&mut plugin, size).unwrap();
        Some([size.width, size.height])
    }

    pub fn destroy(&mut self) {
        if self.is_open {
            self.ext.destroy(&mut self.instance.plugin_handle());
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
