use clack_extensions::gui::{GuiApiType, GuiConfiguration, PluginGui};
use clack_host::plugin::PluginMainThreadHandle;
use generic_daw_utils::NoDebug;

#[derive(Clone, Copy, Debug)]
pub enum Gui {
	Floating {
		ext: NoDebug<PluginGui>,
	},
	Embedded {
		ext: NoDebug<PluginGui>,
		can_resize: Option<bool>,
		scale_factor: f32,
	},
	None,
}

impl Gui {
	pub fn new(plugin: &mut PluginMainThreadHandle<'_>) -> Self {
		plugin
			.get_extension::<PluginGui>()
			.map_or(Self::None, |ext| {
				let mut config = GuiConfiguration {
					api_type: const { GuiApiType::default_for_current_platform().unwrap() },
					is_floating: false,
				};

				if ext.is_api_supported(plugin, config) {
					Self::Embedded {
						ext: ext.into(),
						can_resize: None,
						scale_factor: 1.0,
					}
				} else {
					config.is_floating = true;
					if ext.is_api_supported(plugin, config) {
						Self::Floating { ext: ext.into() }
					} else {
						Self::None
					}
				}
			})
	}

	pub fn ext(&self) -> Option<PluginGui> {
		match self {
			Self::Floating { ext } | Self::Embedded { ext, .. } => Some(ext.0),
			Self::None => None,
		}
	}
}
