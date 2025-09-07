use crate::{
	API_TYPE, EventImpl, PluginDescriptor, PluginId, audio_processor::AudioThreadMessage, gui::Gui,
	host::Host, params::Param, size::Size,
};
use async_channel::Sender;
use clack_extensions::{
	gui::{GuiConfiguration, GuiSize, Window as ClapWindow},
	latency::PluginLatency,
	params::ParamInfoFlags,
	render::{PluginRender, RenderMode},
	state::PluginState,
	timer::{PluginTimer, TimerId},
};
use clack_host::prelude::*;
use generic_daw_utils::NoDebug;
use log::warn;
use raw_window_handle::RawWindowHandle;
use std::{io::Cursor, panic};

#[derive(Debug)]
pub struct Plugin<Event: EventImpl> {
	instance: NoDebug<PluginInstance<Host>>,
	gui: Gui,
	descriptor: PluginDescriptor,
	id: PluginId,
	sender: Sender<AudioThreadMessage<Event>>,
	params: NoDebug<Box<[Param]>>,
	is_open: bool,
}

impl<Event: EventImpl> Plugin<Event> {
	#[must_use]
	pub fn new(
		instance: PluginInstance<Host>,
		gui: Gui,
		descriptor: PluginDescriptor,
		sender: Sender<AudioThreadMessage<Event>>,
		id: PluginId,
		params: Box<[Param]>,
	) -> Self {
		Self {
			instance: instance.into(),
			gui,
			descriptor,
			id,
			sender,
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
		!matches!(self.gui, Gui::None)
	}

	#[must_use]
	pub fn is_floating(&self) -> bool {
		matches!(self.gui, Gui::Floating { .. })
	}

	pub fn set_scale(&mut self, scale: f32) {
		let Gui::Embedded {
			ext, scale_factor, ..
		} = &mut self.gui
		else {
			panic!("called \"set_scale\" on a non-embedded gui")
		};

		if !API_TYPE.uses_logical_size()
			&& let Err(err) = ext.set_scale(&mut self.instance.plugin_handle(), scale.into())
		{
			// If I unwrap here, vital doesn't load. Why?
			warn!("{}: {err}", self.descriptor);
		} else {
			*scale_factor = scale;
		}
	}

	#[must_use]
	pub fn can_resize(&mut self) -> bool {
		let Gui::Embedded {
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

	pub fn request_restart(&self) {
		self.sender
			.try_send(AudioThreadMessage::RequestRestart)
			.unwrap();
	}

	#[must_use]
	pub fn params(&self) -> impl DoubleEndedIterator<Item = &Param> {
		self.params
			.iter()
			.filter(|param| !param.flags.contains(ParamInfoFlags::IS_HIDDEN))
	}

	pub fn update_param(&mut self, param_id: ClapId, value: f32) {
		self.params
			.iter_mut()
			.find(|param| param.id == param_id)
			.unwrap()
			.update_with_value(value, &mut self.instance.plugin_handle());
	}

	pub fn rescan_values(&mut self) {
		for param in &mut *self.params {
			param.rescan_value(&mut self.instance.plugin_handle());
		}
	}

	pub fn tick_timer(&mut self, id: u32) {
		self.instance
			.plugin_handle()
			.get_extension::<PluginTimer>()
			.unwrap()
			.on_timer(&mut self.instance.plugin_handle(), TimerId(id));
	}

	pub fn create(&mut self) {
		self.destroy();

		let Some(ext) = self.gui.ext() else {
			return;
		};

		let config = GuiConfiguration {
			api_type: API_TYPE,
			is_floating: self.is_floating(),
		};

		ext.create(&mut self.instance.plugin_handle(), config)
			.unwrap();
	}

	/// # SAFETY
	/// The underlying window must remain valid for the lifetime of this plugin instance's gui.
	pub unsafe fn set_parent(&mut self, window_handle: RawWindowHandle) {
		let Gui::Embedded { ext, .. } = self.gui else {
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
	pub fn get_size(&mut self) -> Option<Size> {
		let Gui::Embedded {
			ext, scale_factor, ..
		} = self.gui
		else {
			return None;
		};

		let GuiSize { width, height } = ext.get_size(&mut self.instance.plugin_handle())?;
		Some(Size::from_native((width as f32, height as f32)).ensure_logical(scale_factor))
	}

	#[must_use]
	pub fn resize(&mut self, size: Size) -> Option<Size> {
		let Gui::Embedded {
			ext,
			can_resize,
			scale_factor,
		} = self.gui
		else {
			return None;
		};

		if !can_resize? {
			return None;
		}

		let (width, height) = size.to_native(scale_factor);
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
		Some(Size::from_native((width as f32, height as f32)).ensure_logical(scale_factor))
	}

	pub fn send_event(&self, event: Event) {
		self.sender
			.try_send(AudioThreadMessage::Event(event))
			.unwrap();
	}

	pub fn latency_changed(&mut self) {
		let latency = self
			.instance
			.plugin_handle()
			.get_extension::<PluginLatency>()
			.unwrap()
			.get(&mut self.instance.plugin_handle());

		self.sender
			.try_send(AudioThreadMessage::LatencyChanged(latency))
			.unwrap();
	}

	pub fn set_realtime(&mut self, realtime: bool) {
		if let Some(render) = self
			.instance
			.plugin_handle()
			.get_extension::<PluginRender>()
		{
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

	#[must_use]
	pub fn get_state(&mut self) -> Option<Vec<u8>> {
		let mut buf = Vec::new();

		self.instance
			.plugin_handle()
			.get_extension::<PluginState>()?
			.save(&mut self.instance.plugin_handle(), &mut buf)
			.ok()?;

		Some(buf)
	}

	pub fn set_state(&mut self, buf: &[u8]) {
		self.instance
			.plugin_handle()
			.get_extension::<PluginState>()
			.unwrap()
			.load(&mut self.instance.plugin_handle(), &mut Cursor::new(buf))
			.unwrap();
	}
}

impl<Event: EventImpl> Drop for Plugin<Event> {
	fn drop(&mut self) {
		self.destroy();
	}
}
