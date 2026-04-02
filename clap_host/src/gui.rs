use crate::{API_TYPE, host::Host};
use clack_extensions::gui::GuiConfiguration;
use clack_host::prelude::*;

#[derive(Clone, Copy, Debug)]
pub enum Gui {
	Floating,
	Embedded {
		can_resize: Option<bool>,
		scale_factor: f32,
	},
	None,
}

impl Gui {
	pub fn new(plugin: &mut PluginInstance<Host>) -> Self {
		plugin
			.access_shared_handler(|s| s.ext.gui.get().copied())
			.map_or(Self::None, |gui| {
				if let Some(config) = gui.get_preferred_api(&mut plugin.plugin_handle())
					&& config.api_type == API_TYPE
				{
					return if config.is_floating {
						Self::Floating
					} else {
						Self::Embedded {
							can_resize: None,
							scale_factor: 1.0,
						}
					};
				}

				let mut config = GuiConfiguration {
					api_type: API_TYPE,
					is_floating: false,
				};

				if gui.is_api_supported(&mut plugin.plugin_handle(), config) {
					Self::Embedded {
						can_resize: None,
						scale_factor: 1.0,
					}
				} else {
					config.is_floating = true;
					if gui.is_api_supported(&mut plugin.plugin_handle(), config) {
						Self::Floating
					} else {
						Self::None
					}
				}
			})
	}
}
