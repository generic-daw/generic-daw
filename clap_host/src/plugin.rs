use crate::{
	API_TYPE, AudioThread, MainThreadMessage, PluginDescriptor, audio_buffers::AudioBuffers,
	audio_processor::AudioProcessor, event_buffers::EventBuffers, gui::Gui, host::Host,
	main_thread::MainThread, param::Param, preset::Preset, shared::Shared, size::Size,
};
use clack_extensions::{
	gui::{GuiConfiguration, GuiSize, Window},
	params::{ParamInfoFlags, ParamRescanFlags},
	render::RenderMode,
	state_context::StateContextType,
	timer::TimerId,
};
use clack_host::prelude::*;
use log::{info, warn};
use raw_window_handle::HasWindowHandle;
use rtrb::{Producer, RingBuffer};
use std::{
	io::Cursor,
	num::NonZero,
	sync::{atomic::Ordering::Relaxed, mpsc::Receiver},
};
use utils::{NoClone, NoDebug};

#[derive(Debug)]
pub struct Plugin {
	gui: Gui,
	params: Box<[Param]>,
	presets: Vec<Preset>,
	instance: NoDebug<PluginInstance<Host>>,
	descriptor: PluginDescriptor,
	producer: Producer<NoDebug<StoppedPluginAudioProcessor<Host>>>,
	is_created: bool,
	is_shown: bool,
}

impl Plugin {
	#[must_use]
	pub fn new(
		descriptor: PluginDescriptor,
		host: HostInfo,
	) -> (Self, AudioThread, Receiver<MainThreadMessage>) {
		// SAFETY:
		// Loading an external library object file is inherently unsafe.
		let entry = unsafe { PluginEntry::load(&*descriptor.path) }.unwrap();

		let (sender, receiver) = std::sync::mpsc::channel();
		let (producer, consumer) = RingBuffer::new(1);

		let mut instance = PluginInstance::new(
			|()| Shared::new(descriptor.clone(), sender.clone()),
			|shared| MainThread::new(shared),
			&entry,
			&descriptor.id,
			&host,
		)
		.unwrap();

		Preset::start_discover(&instance, entry, descriptor.clone(), host, sender);

		let processor = AudioThread::new(descriptor.clone(), consumer);

		let plugin = Self {
			gui: Gui::new(&mut instance),
			params: Param::all(&mut instance).unwrap_or_default(),
			presets: Vec::new(),
			instance: instance.into(),
			descriptor,
			producer,
			is_created: false,
			is_shown: false,
		};

		(plugin, processor, receiver)
	}

	#[must_use]
	pub fn descriptor(&self) -> &PluginDescriptor {
		&self.descriptor
	}

	#[must_use]
	pub fn has_gui(&self) -> bool {
		self.is_floating() || self.is_embedded()
	}

	#[must_use]
	pub fn is_floating(&self) -> bool {
		self.gui == Gui::Floating
	}

	#[must_use]
	pub fn is_embedded(&self) -> bool {
		matches!(self.gui, Gui::Embedded { .. })
	}

	#[must_use]
	pub fn get_scale(&self) -> Option<f32> {
		if let Gui::Embedded { scale_factor, .. } = &self.gui {
			Some(*scale_factor)
		} else {
			None
		}
	}

	#[must_use]
	pub fn set_scale(&mut self, scale: f32) -> f32 {
		self.create();

		let Gui::Embedded { scale_factor, .. } = &mut self.gui else {
			panic!("called \"set_scale\" on a non-embedded gui");
		};

		if API_TYPE.uses_logical_size() {
			*scale_factor = scale;
		} else if let Err(err) = self
			.instance
			.access_shared_handler(|s| *s.ext.gui.get().unwrap())
			.set_scale(&mut self.instance.plugin_handle(), scale.into())
		{
			warn!("{}: {err}", self.descriptor);
		} else {
			*scale_factor = scale;
		}

		*scale_factor
	}

	#[must_use]
	pub fn can_resize(&mut self) -> bool {
		self.create();

		let Gui::Embedded { can_resize, .. } = &mut self.gui else {
			panic!("called \"can_resize\" on a non-embedded gui");
		};

		*can_resize.get_or_insert_with(|| {
			self.instance
				.access_shared_handler(|s| *s.ext.gui.get().unwrap())
				.can_resize(&mut self.instance.plugin_handle())
		})
	}

	pub fn call_on_main_thread_callback(&mut self) {
		self.instance.call_on_main_thread_callback();
	}

	pub fn is_active(&self) -> bool {
		self.instance.is_active()
	}

	pub fn activate(
		&mut self,
		sample_rate: NonZero<u32>,
		frames: NonZero<u32>,
	) -> Result<(), PluginInstanceError> {
		if self
			.instance
			.access_handler_mut(|mt| std::mem::take(&mut mt.params_rescan))
		{
			self.params = Param::all(&mut self.instance).unwrap_or_default();
		}

		let config = PluginAudioConfiguration {
			sample_rate: sample_rate.get().into(),
			min_frames_count: 1,
			max_frames_count: frames.get(),
		};

		let audio_buffers = AudioBuffers::new(&mut self.instance, config);
		let event_buffers = EventBuffers::new(&mut self.instance, &self.params);

		let mut processor = self.instance.activate(
			|shared, _| AudioProcessor::new(shared, audio_buffers, event_buffers),
			config,
		)?;

		if let Some(&latency) = self.instance.access_shared_handler(|s| s.ext.latency.get()) {
			let latency = latency.get(&mut self.instance.plugin_handle());
			processor
				.access_handler_mut(|ap| ap.audio_buffers.as_mut().unwrap().set_latency(latency));
		}

		self.producer.push(NoDebug(processor)).unwrap();

		Ok(())
	}

	pub fn request_deactivate(&self) {
		self.instance
			.access_shared_handler(|s| s.request_deactivate.store(true, Relaxed));
	}

	pub fn deactivate(
		&mut self,
		NoClone(NoDebug(processor)): NoClone<NoDebug<StoppedPluginAudioProcessor<Host>>>,
	) {
		self.instance.deactivate(processor);
	}

	#[must_use]
	pub fn params(&self) -> impl DoubleEndedIterator<Item = &Param> {
		self.params
			.iter()
			.filter(|param| !param.flags.contains(ParamInfoFlags::IS_HIDDEN))
	}

	pub fn adjust_param_value(&mut self, param_id: ClapId, value: f32) -> Option<&Param> {
		let param = self
			.params
			.iter_mut()
			.find(|param| param.id == param_id)
			.unwrap();

		param
			.adjust_value(&mut self.instance, value)
			.then_some(param)
	}

	pub fn rescan_param(&mut self, param_id: ClapId, flags: ParamRescanFlags) {
		self.params
			.iter_mut()
			.find(|param| param.id == param_id)
			.unwrap()
			.rescan(&mut self.instance, flags);
	}

	pub fn rescan_params(&mut self, flags: ParamRescanFlags) {
		for param in &mut *self.params {
			param.rescan(&mut self.instance, flags);
		}
	}

	#[must_use]
	pub fn presets(&self) -> &[Preset] {
		&self.presets
	}

	pub fn preset_discovered(&mut self, preset: Preset) {
		self.presets.push(preset);
	}

	pub fn load_preset(&mut self, preset: usize) {
		if let Err(err) = self
			.instance
			.access_shared_handler(|s| *s.ext.preset_load.get().unwrap())
			.load_from_location(
				&mut self.instance.plugin_handle(),
				(&self.presets[preset].location).into(),
				self.presets[preset].load_key.as_deref(),
			) {
			warn!("{}: {err}", self.descriptor);
		}
	}

	pub fn tick_timer(&mut self, timer_id: TimerId) {
		self.instance
			.access_shared_handler(|s| *s.ext.timer.get().unwrap())
			.on_timer(&mut self.instance.plugin_handle(), timer_id);
	}

	#[must_use]
	pub fn is_created(&self) -> bool {
		self.is_created
	}

	#[must_use]
	pub fn is_shown(&self) -> bool {
		self.is_shown
	}

	pub fn create(&mut self) {
		let config = GuiConfiguration {
			api_type: API_TYPE,
			is_floating: self.is_floating(),
		};

		if !self.is_created
			&& let Some(&gui) = self.instance.access_shared_handler(|s| s.ext.gui.get())
			&& let Err(err) = gui.create(&mut self.instance.plugin_handle(), config)
		{
			warn!("{}: {err}", self.descriptor);
		}

		self.is_created = true;
	}

	pub fn destroy(&mut self) {
		self.hide();

		if self.is_created
			&& let Some(&gui) = self.instance.access_shared_handler(|s| s.ext.gui.get())
		{
			gui.destroy(&mut self.instance.plugin_handle());
		}

		self.is_created = false;
	}

	/// # SAFETY
	/// The underlying window must remain valid for the lifetime of this plugin instance's gui.
	pub unsafe fn set_parent(&mut self, window: impl HasWindowHandle) {
		let Gui::Embedded { .. } = self.gui else {
			panic!("called \"set_parent\" on a non-embedded gui");
		};

		self.create();

		// SAFETY:
		// Ensured by the caller.
		if let Err(err) = unsafe {
			self.instance
				.access_shared_handler(|s| *s.ext.gui.get().unwrap())
				.set_parent(
					&mut self.instance.plugin_handle(),
					Window::from_window(&window).unwrap(),
				)
		} {
			warn!("{}: {err}", self.descriptor);
		}
	}

	/// # SAFETY
	/// The underlying window must remain valid for the lifetime of this plugin instance's gui.
	pub unsafe fn set_transient(&mut self, window: impl HasWindowHandle) {
		let Gui::Floating = self.gui else {
			panic!("called \"set_transient\" on a non-floating gui");
		};

		self.create();

		// SAFETY:
		// Ensured by the caller.
		if let Err(err) = unsafe {
			self.instance
				.access_shared_handler(|s| *s.ext.gui.get().unwrap())
				.set_transient(
					&mut self.instance.plugin_handle(),
					Window::from_window(&window).unwrap(),
				)
		} {
			warn!("{}: {err}", self.descriptor);
		}
	}

	pub fn show(&mut self) {
		self.create();

		if !self.is_shown
			&& let Some(&gui) = self.instance.access_shared_handler(|s| s.ext.gui.get())
			&& let Err(err) = gui.show(&mut self.instance.plugin_handle())
		{
			warn!("{}: {err}", self.descriptor);
		}

		self.is_shown = true;
	}

	pub fn hide(&mut self) {
		if self.is_shown
			&& let Some(&gui) = self.instance.access_shared_handler(|s| s.ext.gui.get())
			&& let Err(err) = gui.hide(&mut self.instance.plugin_handle())
		{
			warn!("{}: {err}", self.descriptor);
		}

		self.is_shown = false;
	}

	#[must_use]
	pub fn get_size(&mut self) -> Option<Size> {
		self.create();

		let GuiSize { width, height } = self
			.instance
			.access_shared_handler(|s| *s.ext.gui.get().unwrap())
			.get_size(&mut self.instance.plugin_handle())?;

		Some(Size::from_native((width as f32, height as f32)))
	}

	#[must_use]
	pub fn resize(&mut self, size: Size) -> Option<Size> {
		let Gui::Embedded { scale_factor, .. } = self.gui else {
			panic!("called \"resize\" on a non-embedded gui");
		};

		if !self.can_resize() {
			return None;
		}

		let (width, height) = size.to_native(scale_factor);
		let size = GuiSize {
			width: width as u32,
			height: height as u32,
		};

		let gui = self
			.instance
			.access_shared_handler(|s| *s.ext.gui.get().unwrap());
		let size = gui.adjust_size(&mut self.instance.plugin_handle(), size)?;
		gui.set_size(&mut self.instance.plugin_handle(), size)
			.inspect_err(|err| warn!("{}: {err}", self.descriptor))
			.ok()?;

		let GuiSize { width, height } = size;
		Some(Size::from_native((width as f32, height as f32)))
	}

	pub fn set_render_mode(&mut self, render_mode: RenderMode) {
		if let Some(&render) = self.instance.access_shared_handler(|s| s.ext.render.get()) {
			if render_mode == RenderMode::Offline
				&& render.has_realtime_requirement(&mut self.instance.plugin_handle())
			{
				warn!("{}: Plugin has hard realtime requirement.", self.descriptor);
			} else if let Err(err) = render.set(&mut self.instance.plugin_handle(), render_mode) {
				warn!("{}: {err}", self.descriptor);
			}
		}
	}

	#[must_use]
	pub fn get_state(&mut self, context_type: StateContextType) -> Option<&[u8]> {
		if self.instance.access_handler(|mt| mt.state.is_none()) {
			let mut buf = Vec::new();

			if let Err(err) = if let Some(&state_context) = self
				.instance
				.access_shared_handler(|s| s.ext.state_context.get())
			{
				state_context.save(&mut self.instance.plugin_handle(), &mut buf, context_type)
			} else if let Some(&state) = self.instance.access_shared_handler(|s| s.ext.state.get())
			{
				state.save(&mut self.instance.plugin_handle(), &mut buf)
			} else {
				return None;
			} {
				warn!("{}: {err}", self.descriptor);
				return None;
			}

			self.instance
				.access_handler_mut(|mt| mt.state = Some(buf.into_boxed_slice()));
		}

		self.instance.access_handler(|mt| mt.state.as_deref())
	}

	pub fn set_state(&mut self, buf: &[u8], context_type: StateContextType) {
		if let Err(err) = if let Some(&state_context) = self
			.instance
			.access_shared_handler(|s| s.ext.state_context.get())
		{
			state_context.load(
				&mut self.instance.plugin_handle(),
				&mut Cursor::new(buf),
				context_type,
			)
		} else if let Some(&state) = self.instance.access_shared_handler(|s| s.ext.state.get()) {
			state.load(&mut self.instance.plugin_handle(), &mut Cursor::new(buf))
		} else {
			return;
		} {
			warn!("{}: {err}", self.descriptor);
			return;
		}

		self.instance
			.access_handler_mut(|mt| mt.state = Some(buf.into()));
	}
}

impl Drop for Plugin {
	fn drop(&mut self) {
		self.destroy();

		if self.instance.try_deactivate() == Err(PluginInstanceError::StillActivatedPlugin) {
			warn!("{}: leaked instance", self.descriptor);
		} else {
			info!("{}: dropped instance", self.descriptor);
		}
	}
}
