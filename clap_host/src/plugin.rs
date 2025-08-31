use crate::{
	PluginDescriptor, PluginId, audio_processor::AudioThreadMessage, host::Host, params::Param,
};
use clack_extensions::{
	gui::{GuiApiType, GuiConfiguration, GuiSize, PluginGui, Window as ClapWindow},
	params::ParamInfoFlags,
	render::RenderMode,
	timer::TimerId,
};
use clack_host::{events::Match, prelude::*};
use generic_daw_utils::NoDebug;
use log::warn;
use raw_window_handle::RawWindowHandle;
use std::{io::Cursor, panic};

#[derive(Debug)]
enum GuiKind {
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

impl GuiKind {
	fn ext(&self) -> Option<PluginGui> {
		match self {
			Self::Floating { ext } | Self::Embedded { ext, .. } => Some(ext.0),
			Self::None => None,
		}
	}
}

#[derive(Debug)]
pub struct Plugin {
	instance: NoDebug<PluginInstance<Host>>,
	gui: GuiKind,
	descriptor: PluginDescriptor,
	id: PluginId,
	params: NoDebug<Box<[Param]>>,
	is_open: bool,
}

impl Plugin {
	#[must_use]
	pub(crate) fn new(
		mut instance: PluginInstance<Host>,
		descriptor: PluginDescriptor,
		id: PluginId,
		params: Box<[Param]>,
	) -> Self {
		let gui = instance
			.access_handler(|mt| mt.gui)
			.map_or(GuiKind::None, |ext| {
				let mut config = GuiConfiguration {
					api_type: const { GuiApiType::default_for_current_platform().unwrap() },
					is_floating: false,
				};

				if ext.is_api_supported(&mut instance.plugin_handle(), config) {
					GuiKind::Embedded {
						ext,
						can_resize: None,
						scale_factor: 1.0,
					}
				} else {
					config.is_floating = true;
					if ext.is_api_supported(&mut instance.plugin_handle(), config) {
						GuiKind::Floating { ext }
					} else {
						GuiKind::None
					}
				}
			});

		Self {
			instance: instance.into(),
			gui,
			descriptor,
			id,
			params: params.into(),
			is_open: false,
		}
	}

	#[must_use]
	pub fn descriptor(&self) -> &PluginDescriptor {
		&self.descriptor
	}

	#[must_use]
	pub fn plugin_id(&self) -> PluginId {
		self.id
	}

	#[must_use]
	pub fn has_gui(&self) -> bool {
		!matches!(self.gui, GuiKind::None)
	}

	#[must_use]
	pub fn is_floating(&self) -> bool {
		matches!(self.gui, GuiKind::Floating { .. })
	}

	pub fn set_scale(&mut self, scale: f32) {
		let GuiKind::Embedded {
			ext, scale_factor, ..
		} = &mut self.gui
		else {
			panic!("called \"set_scale\" on a non-embedded gui")
		};

		*scale_factor = scale;
		ext.set_scale(&mut self.instance.plugin_handle(), scale.into())
			.unwrap();
	}

	#[must_use]
	pub fn can_resize(&mut self) -> bool {
		let GuiKind::Embedded {
			ext, can_resize, ..
		} = &mut self.gui
		else {
			panic!("called \"can_resize\" on a non-embedded gui")
		};

		*can_resize.get_or_insert_with(|| ext.can_resize(&mut self.instance.plugin_handle()))
	}

	pub fn call_on_main_thread_callback(&mut self) {
		self.instance.call_on_main_thread_callback();
	}

	#[must_use]
	pub fn params(&self) -> impl DoubleEndedIterator<Item = &Param> {
		self.params
			.iter()
			.filter(|param| !param.flags.contains(ParamInfoFlags::IS_HIDDEN))
	}

	pub fn rescan_values(&mut self, id: impl Into<Match<ClapId>>) {
		let ext = self.instance.access_handler(|mt| mt.params).unwrap().0;

		match id.into() {
			Match::Specific(id) => {
				if let Some(param) = self.params.iter_mut().find(|param| param.id == id) {
					param.rescan_value(&mut self.instance.plugin_handle(), ext);
				}
			}
			Match::All => {
				for param in &mut *self.params {
					param.rescan_value(&mut self.instance.plugin_handle(), ext);
				}
			}
		}
	}

	pub fn tick_timer(&mut self, id: u32) {
		self.instance
			.access_handler(|mt| mt.timers)
			.unwrap()
			.on_timer(&mut self.instance.plugin_handle(), TimerId(id));
	}

	pub fn create(&mut self) {
		self.destroy();

		let Some(ext) = self.gui.ext() else {
			return;
		};

		let config = GuiConfiguration {
			api_type: const { GuiApiType::default_for_current_platform().unwrap() },
			is_floating: self.is_floating(),
		};

		ext.create(&mut self.instance.plugin_handle(), config)
			.unwrap();
	}

	/// # SAFETY
	/// The underlying window must remain valid for the lifetime of this plugin instance's gui.
	pub unsafe fn set_parent(&mut self, window_handle: RawWindowHandle) {
		let GuiKind::Embedded { ext, .. } = self.gui else {
			panic!("called \"set_parent\" on a non-embedded gui");
		};

		// SAFETY:
		// Ensured by the caller.
		unsafe {
			ext.set_parent(
				&mut self.instance.plugin_handle(),
				ClapWindow::from_window_handle(window_handle).unwrap(),
			)
			.unwrap();
		}
	}

	pub fn show(&mut self) {
		if !self.is_open
			&& let Some(ext) = self.gui.ext()
			&& let Err(err) = ext.show(&mut self.instance.plugin_handle())
		{
			// If I unwrap here, nih-plug plugins don't load. Why?
			warn!("{}: {err}", self.descriptor);
		}

		self.is_open = true;
	}

	pub fn destroy(&mut self) {
		if self.is_open
			&& let Some(ext) = self.gui.ext()
		{
			ext.destroy(&mut self.instance.plugin_handle());
		}

		self.is_open = false;
	}

	#[must_use]
	pub fn get_size(&mut self) -> Option<[f32; 2]> {
		let GuiKind::Embedded {
			ext, scale_factor, ..
		} = self.gui
		else {
			return None;
		};

		let GuiSize { width, height } = ext.get_size(&mut self.instance.plugin_handle())?;
		let (mut width, mut height) = (width as f32, height as f32);

		if !const { GuiApiType::default_for_current_platform().unwrap() }.uses_logical_size() {
			width *= scale_factor;
			height *= scale_factor;
		}

		#[expect(clippy::tuple_array_conversions)]
		Some([width, height])
	}

	#[must_use]
	pub fn resize(&mut self, mut width: f32, mut height: f32) -> Option<[f32; 2]> {
		let GuiKind::Embedded {
			ext,
			scale_factor,
			can_resize,
		} = self.gui
		else {
			return None;
		};

		if !can_resize? {
			return None;
		}

		if !const { GuiApiType::default_for_current_platform().unwrap() }.uses_logical_size() {
			width /= scale_factor;
			height /= scale_factor;
		}

		let size = GuiSize {
			width: width as u32,
			height: height as u32,
		};

		let size = ext
			.adjust_size(&mut self.instance.plugin_handle(), size)
			.unwrap();
		ext.set_size(&mut self.instance.plugin_handle(), size)
			.unwrap();

		let GuiSize { width, height } = size;
		let (mut width, mut height) = (width as f32, height as f32);

		if !const { GuiApiType::default_for_current_platform().unwrap() }.uses_logical_size() {
			width *= scale_factor;
			height *= scale_factor;
		}

		#[expect(clippy::tuple_array_conversions)]
		Some([width, height])
	}

	pub fn latency_changed(&mut self) {
		let latency = self
			.instance
			.access_handler(|mt| mt.latency)
			.unwrap()
			.get(&mut self.instance.plugin_handle());

		self.instance.access_shared_handler(|sh| {
			sh.audio_sender
				.try_send(AudioThreadMessage::LatencyChanged(latency))
				.unwrap();
		});
	}

	pub fn set_realtime(&mut self, realtime: bool) {
		if let Some(render) = self.instance.access_handler(|mt| mt.render) {
			render
				.set(
					&mut self.instance.plugin_handle(),
					if realtime {
						RenderMode::Realtime
					} else {
						RenderMode::Offline
					},
				)
				.unwrap();
		}
	}

	pub fn get_state(&mut self) -> Option<Vec<u8>> {
		let mut buf = Vec::new();

		self.instance
			.access_handler(|mt| mt.state)?
			.save(&mut self.instance.plugin_handle(), &mut buf)
			.ok()?;

		Some(buf)
	}

	pub fn set_state(&mut self, buf: &[u8]) {
		self.instance
			.access_handler(|mt| mt.state)
			.unwrap()
			.load(&mut self.instance.plugin_handle(), &mut Cursor::new(buf))
			.unwrap();
	}
}

impl Drop for Plugin {
	fn drop(&mut self) {
		self.destroy();
	}
}
