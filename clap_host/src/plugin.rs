use crate::{
	API_TYPE, AudioProcessor, EventImpl, MainThreadMessage, ParamInfoFlags, ParamRescanFlags,
	PluginDescriptor, StateContextType, TimerId, audio_buffers::AudioBuffers,
	audio_processor::AudioThreadMessage, audio_thread::AudioThread, event_buffers::EventBuffers,
	gui::Gui, host::Host, main_thread::MainThread, param::Param, preset::Preset, shared::Shared,
	size::Size,
};
#[cfg(unix)]
use clack_extensions::posix_fd::FdFlags;
use clack_extensions::{
	gui::{GuiConfiguration, GuiSize, Window},
	render::RenderMode,
};
use clack_host::prelude::*;
use log::{info, warn};
use raw_window_handle::HasWindowHandle;
use rtrb::{Producer, PushError, RingBuffer};
#[cfg(unix)]
use std::os::fd::RawFd;
use std::{
	io::Cursor,
	num::NonZero,
	sync::{atomic::Ordering::Relaxed, mpsc::Receiver},
};
use utils::{NoClone, NoDebug};

#[derive(Debug)]
pub struct Plugin<Event: EventImpl> {
	gui: Gui,
	params: Box<[Param]>,
	presets: Box<[Preset]>,
	instance: NoDebug<PluginInstance<Host>>,
	descriptor: PluginDescriptor,
	producer: Producer<AudioThreadMessage<Event>>,
	config: PluginAudioConfiguration,
	last_state: Option<Box<[u8]>>,
	is_created: bool,
	is_shown: bool,
}

impl<Event: EventImpl> Plugin<Event> {
	#[must_use]
	pub fn new(
		descriptor: PluginDescriptor,
		sample_rate: NonZero<u32>,
		frames: NonZero<u32>,
		host: &HostInfo,
	) -> (AudioProcessor<Event>, Self, Receiver<MainThreadMessage>) {
		// SAFETY:
		// Loading an external library object file is inherently unsafe.
		let bundle = unsafe { PluginBundle::load(&*descriptor.path) }.unwrap();

		let (shared_sender, receiver) = std::sync::mpsc::channel();
		let (producer, audio_consumer) = RingBuffer::new(frames.get() as usize);

		let mut instance = PluginInstance::new(
			|()| Shared::new(descriptor.clone(), shared_sender),
			|shared| MainThread::new(shared),
			&bundle,
			&descriptor.id,
			host,
		)
		.unwrap();

		let config = PluginAudioConfiguration {
			sample_rate: sample_rate.get().into(),
			min_frames_count: 1,
			max_frames_count: frames.get(),
		};

		(
			AudioProcessor::new(
				descriptor.clone(),
				AudioBuffers::new(&mut instance, config),
				EventBuffers::new(&mut instance),
				audio_consumer,
			),
			Self {
				gui: Gui::new(&mut instance),
				params: Param::all(&mut instance).unwrap_or_default(),
				presets: Preset::all(&instance, &bundle, &descriptor, host).unwrap_or_default(),
				instance: instance.into(),
				descriptor,
				producer,
				config,
				last_state: None,
				is_created: false,
				is_shown: false,
			},
			receiver,
		)
	}

	fn send(&mut self, mut message: AudioThreadMessage<Event>) {
		while let Err(PushError::Full(msg)) = self.producer.push(message) {
			message = msg;
			std::thread::yield_now();
		}
	}

	#[must_use]
	pub fn descriptor(&self) -> &PluginDescriptor {
		&self.descriptor
	}

	#[must_use]
	pub fn has_gui(&self) -> bool {
		!matches!(self.gui, Gui::None)
	}

	#[must_use]
	pub fn is_floating(&self) -> bool {
		matches!(self.gui, Gui::Floating)
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

		if !API_TYPE.uses_logical_size() {
			if let Err(err) = self
				.instance
				.access_shared_handler(|s| *s.ext.gui.get().unwrap())
				.set_scale(&mut self.instance.plugin_handle(), scale.into())
			{
				warn!("{}: {err}", self.descriptor);
			} else {
				*scale_factor = scale;
			}
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
		if self
			.instance
			.access_shared_handler(|s| s.needs_callback.swap(false, Relaxed))
		{
			self.instance.call_on_main_thread_callback();
		}
	}

	#[cfg(unix)]
	pub fn on_fd(&mut self, fd: RawFd, flags: FdFlags) {
		self.instance
			.access_shared_handler(|s| *s.ext.posix_fd.get().unwrap())
			.on_fd(&mut self.instance.plugin_handle(), fd, flags);
	}

	pub fn activate(&mut self) {
		if self
			.instance
			.access_handler_mut(|mt| std::mem::take(&mut mt.params_rescan))
		{
			self.params = Param::all(&mut self.instance).unwrap_or_default();
		}

		let processor = self
			.instance
			.activate(|shared, _| AudioThread::new(shared), self.config)
			.unwrap()
			.into();

		let latency = if self
			.instance
			.access_handler_mut(|mt| std::mem::take(&mut mt.latency_changed))
			&& let Some(&ext) = self.instance.access_shared_handler(|s| s.ext.latency.get())
		{
			Some(ext.get(&mut self.instance.plugin_handle()))
		} else {
			None
		};

		self.send(AudioThreadMessage::Activated(NoDebug(processor), latency));
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
			&& let Some(&ext) = self.instance.access_shared_handler(|s| s.ext.gui.get())
			&& let Err(err) = ext.create(&mut self.instance.plugin_handle(), config)
		{
			warn!("{}: {err}", self.descriptor);
		}

		self.is_created = true;
	}

	pub fn destroy(&mut self) {
		self.hide();

		if self.is_created
			&& let Some(&ext) = self.instance.access_shared_handler(|s| s.ext.gui.get())
		{
			ext.destroy(&mut self.instance.plugin_handle());
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
			&& let Some(&ext) = self.instance.access_shared_handler(|s| s.ext.gui.get())
			&& let Err(err) = ext.show(&mut self.instance.plugin_handle())
		{
			warn!("{}: {err}", self.descriptor);
		}

		self.is_shown = true;
	}

	pub fn hide(&mut self) {
		if self.is_shown
			&& let Some(&ext) = self.instance.access_shared_handler(|s| s.ext.gui.get())
			&& let Err(err) = ext.hide(&mut self.instance.plugin_handle())
		{
			warn!("{}: {err}", self.descriptor);
		}

		self.is_shown = false;
	}

	#[must_use]
	pub fn get_size(&mut self) -> Option<Size> {
		self.create();

		if !matches!(self.gui, Gui::Embedded { .. }) {
			return None;
		}

		let GuiSize { width, height } = self
			.instance
			.access_shared_handler(|s| *s.ext.gui.get().unwrap())
			.get_size(&mut self.instance.plugin_handle())?;
		Some(Size::from_native((width as f32, height as f32)))
	}

	#[must_use]
	pub fn resize(&mut self, size: Size) -> Option<Size> {
		let Gui::Embedded { scale_factor, .. } = self.gui else {
			return None;
		};

		if !self.can_resize() {
			return None;
		}

		let (width, height) = size.to_native(scale_factor);
		let size = GuiSize {
			width: width as u32,
			height: height as u32,
		};

		let ext = self
			.instance
			.access_shared_handler(|s| *s.ext.gui.get().unwrap());
		let size = ext.adjust_size(&mut self.instance.plugin_handle(), size)?;
		ext.set_size(&mut self.instance.plugin_handle(), size)
			.inspect_err(|err| warn!("{}: {err}", self.descriptor))
			.ok()?;

		let GuiSize { width, height } = size;
		Some(Size::from_native((width as f32, height as f32)))
	}

	pub fn send_event(&mut self, event: Event) {
		self.send(AudioThreadMessage::Event(event));
	}

	pub fn set_realtime(&mut self, realtime: bool) {
		self.send(AudioThreadMessage::SetRealtime(realtime));

		if let Some(&render) = self.instance.access_shared_handler(|s| s.ext.render.get())
			&& let Err(err) = render.set(
				&mut self.instance.plugin_handle(),
				if realtime {
					RenderMode::Realtime
				} else {
					RenderMode::Offline
				},
			) {
			warn!("{}: {err}", self.descriptor);
		}
	}

	#[must_use]
	pub fn get_state(&mut self, context_type: StateContextType) -> Option<&[u8]> {
		if self.last_state.is_none() || self.instance.access_handler(|mt| mt.state_mark_dirty) {
			let mut buf = Vec::new();

			if let Err(err) = if let Some(&ext) = self
				.instance
				.access_shared_handler(|s| s.ext.state_context.get())
			{
				ext.save(&mut self.instance.plugin_handle(), &mut buf, context_type)
			} else {
				self.instance
					.access_shared_handler(|s| s.ext.state.get().copied())?
					.save(&mut self.instance.plugin_handle(), &mut buf)
			} {
				warn!("{}: {err}", self.descriptor);
				return None;
			}

			self.instance
				.access_handler_mut(|mt| mt.state_mark_dirty = false);
			self.last_state = Some(buf.into_boxed_slice());
		}

		self.last_state.as_deref()
	}

	pub fn set_state(&mut self, buf: &[u8], context_type: StateContextType) {
		if let Err(err) = if let Some(&ext) = self
			.instance
			.access_shared_handler(|s| s.ext.state_context.get())
		{
			ext.load(
				&mut self.instance.plugin_handle(),
				&mut Cursor::new(buf),
				context_type,
			)
		} else {
			self.instance
				.access_shared_handler(|s| *s.ext.state.get().unwrap())
				.load(&mut self.instance.plugin_handle(), &mut Cursor::new(buf))
		} {
			warn!("{}: {err}", self.descriptor);
			return;
		}

		self.last_state = Some(buf.into());
	}
}

impl<Event: EventImpl> Drop for Plugin<Event> {
	fn drop(&mut self) {
		self.destroy();

		if matches!(
			self.instance.try_deactivate(),
			Err(PluginInstanceError::StillActivatedPlugin)
		) {
			warn!("{}: leaked instance", self.descriptor);
		} else {
			info!("{}: dropped instance", self.descriptor);
		}
	}
}
