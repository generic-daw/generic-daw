use clack_extensions::gui::{
    GuiApiType, GuiConfiguration, GuiSize, PluginGui, Window as ClapWindow,
};
use clack_host::prelude::*;
use std::fmt::{Debug, Formatter};
use winit::{
    dpi::{LogicalSize, PhysicalSize, Size},
    raw_window_handle::RawWindowHandle,
};

#[derive(Clone, Copy)]
pub struct GuiExt {
    plugin_gui: PluginGui,
    pub configuration: Option<GuiConfiguration<'static>>,
    is_open: bool,
    is_resizeable: bool,
}

impl Debug for GuiExt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiExt")
            .field("configuration", &self.configuration)
            .field("is_open", &self.is_open)
            .field("is_resizeable", &self.is_resizeable)
            .finish_non_exhaustive()
    }
}

impl GuiExt {
    pub fn new(plugin_gui: PluginGui, instance: &mut PluginMainThreadHandle<'_>) -> Self {
        Self {
            plugin_gui,
            configuration: Self::negotiate_configuration(&plugin_gui, instance),
            is_open: false,
            is_resizeable: false,
        }
    }

    fn negotiate_configuration(
        gui: &PluginGui,
        plugin: &mut PluginMainThreadHandle<'_>,
    ) -> Option<GuiConfiguration<'static>> {
        let api_type = GuiApiType::default_for_current_platform()?;
        let mut config = GuiConfiguration {
            api_type,
            is_floating: false,
        };

        if gui.is_api_supported(plugin, config) {
            Some(config)
        } else {
            config.is_floating = true;
            if gui.is_api_supported(plugin, config) {
                Some(config)
            } else {
                None
            }
        }
    }

    pub fn gui_size_to_winit_size(&self, size: GuiSize) -> Size {
        let Some(GuiConfiguration { api_type, .. }) = self.configuration else {
            panic!("Called gui_size_to_winit_size on incompatible plugin")
        };

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

    pub fn needs_floating(&self) -> Option<bool> {
        self.configuration
            .map(|GuiConfiguration { is_floating, .. }| is_floating)
    }

    pub fn open_floating(&mut self, plugin: &mut PluginMainThreadHandle<'_>) {
        let Some(configuration) = self.configuration.filter(|c| c.is_floating) else {
            panic!("Called open_floating on incompatible plugin")
        };

        self.plugin_gui.create(plugin, configuration).unwrap();
        self.plugin_gui.suggest_title(plugin, c"");
        self.plugin_gui.show(plugin).unwrap();

        self.is_resizeable = self.plugin_gui.can_resize(plugin);
        self.is_open = true;
    }

    pub fn open_embedded(
        &mut self,
        plugin: &mut PluginMainThreadHandle<'_>,
        window_handle: RawWindowHandle,
    ) {
        let Some(configuration) = self.configuration.filter(|c| !c.is_floating) else {
            panic!("Called open_embedded on incompatible plugin")
        };

        self.plugin_gui.create(plugin, configuration).unwrap();

        let window = ClapWindow::from_window_handle(window_handle).unwrap();

        // SAFETY:
        // We destroy the plugin ui just before the window is closed (see generic_front/clap_host.rs)
        unsafe { self.plugin_gui.set_parent(plugin, window) }.unwrap();

        self.plugin_gui.show(plugin).unwrap();

        self.is_resizeable = self.plugin_gui.can_resize(plugin);
        self.is_open = true;
    }

    pub fn resize(&self, plugin: &mut PluginMainThreadHandle<'_>, size: Size) -> Size {
        let uses_logical_pixels = self.configuration.unwrap().api_type.uses_logical_size();

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

        if !self.is_resizeable {
            let forced_size = self.plugin_gui.get_size(plugin).unwrap_or(size);

            return self.gui_size_to_winit_size(forced_size);
        }

        let working_size = self.plugin_gui.adjust_size(plugin, size).unwrap_or(size);
        self.plugin_gui.set_size(plugin, working_size).unwrap();

        self.gui_size_to_winit_size(working_size)
    }

    pub fn destroy(mut self, plugin: &mut PluginMainThreadHandle<'_>) {
        if self.is_open {
            self.plugin_gui.destroy(plugin);
            self.is_open = false;
        }
    }
}
