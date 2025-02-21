use crate::{PluginId, host::Host, timer::Timers};
use clack_extensions::{
    gui::{GuiApiType, GuiConfiguration, GuiSize, PluginGui, Window as ClapWindow},
    timer::PluginTimer,
};
use clack_host::prelude::*;
use dpi::{LogicalSize, PhysicalSize, Size};
use raw_window_handle::RawWindowHandle;
use std::{
    cell::RefCell,
    fmt::{Debug, Formatter},
    rc::Rc,
};

pub struct GuiExt {
    instance: PluginInstance<Host>,
    plugin_gui: PluginGui,
    id: PluginId,
    configuration: Option<GuiConfiguration<'static>>,
    is_open: bool,
    can_resize: bool,
}

impl Debug for GuiExt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiExt")
            .field("configuration", &self.configuration)
            .field("is_open", &self.is_open)
            .field("is_resizeable", &self.can_resize)
            .finish_non_exhaustive()
    }
}

impl GuiExt {
    #[must_use]
    pub fn new(plugin_gui: PluginGui, mut instance: PluginInstance<Host>) -> Self {
        let api_type = GuiApiType::default_for_current_platform().unwrap();
        let mut config = GuiConfiguration {
            api_type,
            is_floating: false,
        };

        let mut plugin = instance.plugin_handle();

        let configuration = if plugin_gui.is_api_supported(&mut plugin, config) {
            Some(config)
        } else {
            config.is_floating = true;
            if plugin_gui.is_api_supported(&mut plugin, config) {
                Some(config)
            } else {
                None
            }
        };

        Self {
            instance,
            plugin_gui,
            id: PluginId::unique(),
            configuration,
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
            .map(|size| self.gui_size_to_dpi_size(size).to_logical(1.0))
            .map(|size| [size.width, size.height])
    }

    pub fn call_on_main_thread_callback(&mut self) {
        self.instance.call_on_main_thread_callback();
    }

    pub fn plugin_handle(&mut self) -> PluginMainThreadHandle<'_> {
        self.instance.plugin_handle()
    }

    #[must_use]
    pub fn timers(&self) -> Option<(Rc<RefCell<Timers>>, PluginTimer)> {
        self.instance
            .access_handler(|h| h.timer_support.map(|ext| (h.timers.clone(), ext)))
    }

    #[must_use]
    pub fn gui_size_to_dpi_size(&self, size: GuiSize) -> Size {
        let api_type = self.configuration.unwrap().api_type;

        if api_type.uses_logical_size() {
            LogicalSize {
                width: size.width,
                height: size.height,
            }
            .into()
        } else {
            PhysicalSize {
                width: size.width,
                height: size.height,
            }
            .into()
        }
    }
    #[must_use]
    pub fn needs_floating(&self) -> Option<bool> {
        self.configuration
            .map(|configuration| configuration.is_floating)
    }

    #[must_use]
    pub fn can_resize(&self) -> bool {
        self.can_resize
    }

    pub fn open_floating(&mut self) {
        let configuration = self.configuration.filter(|c| c.is_floating).unwrap();
        let mut plugin = self.instance.plugin_handle();

        self.plugin_gui.create(&mut plugin, configuration).unwrap();
        self.plugin_gui.suggest_title(&mut plugin, c"");
        self.plugin_gui.show(&mut plugin).unwrap();

        self.can_resize = self.plugin_gui.can_resize(&mut plugin);
        self.is_open = true;
    }

    pub fn open_embedded(&mut self, window_handle: RawWindowHandle) {
        let configuration = self.configuration.filter(|c| !c.is_floating).unwrap();
        let mut plugin = self.instance.plugin_handle();

        self.plugin_gui.create(&mut plugin, configuration).unwrap();

        let window = ClapWindow::from_window_handle(window_handle).unwrap();

        // SAFETY:
        // We destroy the plugin ui just before the window is closed
        unsafe { self.plugin_gui.set_parent(&mut plugin, window) }.unwrap();

        self.plugin_gui.show(&mut plugin).unwrap();

        self.can_resize = self.plugin_gui.can_resize(&mut plugin);
        self.is_open = true;
    }

    #[must_use]
    pub fn resize(&mut self, width: u32, height: u32) -> [u32; 2] {
        let uses_logical_pixels = self.configuration.unwrap().api_type.uses_logical_size();
        let mut plugin = self.instance.plugin_handle();
        let size = Size::Physical(PhysicalSize::new(width, height));

        let size = if uses_logical_pixels {
            let size = size.to_logical(1.0);
            GuiSize {
                width: size.width,
                height: size.height,
            }
        } else {
            let size = size.to_physical(1.0);
            GuiSize {
                width: size.width,
                height: size.height,
            }
        };

        if self.can_resize {
            let size = self
                .plugin_gui
                .adjust_size(&mut plugin, size)
                .unwrap_or(size);
            self.plugin_gui.set_size(&mut plugin, size).unwrap();
        };

        let size = self.plugin_gui.get_size(&mut plugin).unwrap_or(size);
        let size = self.gui_size_to_dpi_size(size).to_logical(1.0);
        [size.width, size.height]
    }

    pub fn destroy(&mut self) {
        if self.is_open {
            self.plugin_gui.destroy(&mut self.instance.plugin_handle());
            self.is_open = false;
        }
    }
}
