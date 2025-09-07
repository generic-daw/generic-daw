use crate::host::Host;
use clack_extensions::gui::{GuiApiType, GuiConfiguration};
use clack_host::plugin::PluginInstance;

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
			.access_shared_handler(|s| s.gui.get().copied())
			.map_or(Self::None, |ext| {
				let mut config = GuiConfiguration {
					api_type: const { GuiApiType::default_for_current_platform().unwrap() },
					is_floating: false,
				};

				if ext.is_api_supported(&mut plugin.plugin_handle(), config) {
					Self::Embedded {
						can_resize: None,
						scale_factor: 1.0,
					}
				} else {
					config.is_floating = true;
					if ext.is_api_supported(&mut plugin.plugin_handle(), config) {
						Self::Floating
					} else {
						Self::None
					}
				}
			})
	}
}
