use crate::{
	API_TYPE, AudioProcessor, EventImpl, MainThreadMessage, PluginDescriptor, PluginId,
	audio_buffers::AudioBuffers, audio_processor::AudioThreadMessage, audio_thread::AudioThread,
	event_buffers::EventBuffers, gui::Gui, host::Host, main_thread::MainThread, params::Param,
	shared::Shared, size::Size,
};
use clack_extensions::{
	gui::{GuiConfiguration, GuiSize, Window as ClapWindow},
	params::{ParamInfoFlags, ParamRescanFlags},
	render::RenderMode,
	timer::TimerId,
};
use clack_host::{prelude::*, process::PluginAudioProcessor};
use generic_daw_utils::{NoClone, NoDebug};
use log::{info, warn};
use raw_window_handle::RawWindowHandle;
use rtrb::{Producer, RingBuffer};
use std::{
	io::Cursor,
	sync::{atomic::Ordering::Relaxed, mpsc::Receiver},
};

#[derive(Debug)]
pub struct Plugin<Event: EventImpl> {
	gui: Gui,
	params: Box<[Param]>,
	instance: NoDebug<PluginInstance<Host>>,
	descriptor: PluginDescriptor,
	id: PluginId,
	sender: Producer<AudioThreadMessage<Event>>,
	config: PluginAudioConfiguration,
	is_open: bool,
}

impl<Event: EventImpl> Plugin<Event> {
	#[must_use]
	pub fn new(
		bundle: &PluginBundle,
		descriptor: PluginDescriptor,
		sample_rate: u32,
		frames: u32,
		host: &HostInfo,
	) -> (AudioProcessor<Event>, Self, Receiver<MainThreadMessage>) {
		let (shared_sender, receiver) = std::sync::mpsc::channel();
		let (sender, audio_receiver) = RingBuffer::new(frames as usize);

		let mut instance = PluginInstance::new(
			|()| Shared::new(descriptor.clone(), shared_sender),
			|shared| MainThread::new(shared),
			bundle,
			&descriptor.id,
			host,
		)
		.unwrap();

		let config = PluginAudioConfiguration {
			sample_rate: sample_rate.into(),
			min_frames_count: 1,
			max_frames_count: frames,
		};
		let id = PluginId::unique();

		(
			AudioProcessor::new(
				descriptor.clone(),
				id,
				AudioBuffers::new(&mut instance, config),
				EventBuffers::new(&mut instance),
				audio_receiver,
			),
			Self {
				gui: Gui::new(&mut instance),
				params: Param::all(&mut instance).unwrap_or_default(),
				instance: instance.into(),
				descriptor,
				id,
				sender,
				config,
				is_open: false,
			},
			receiver,
		)
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
		matches!(self.gui, Gui::Floating)
	}

	pub fn set_scale(&mut self, scale: f32) {
		let ext = self
			.instance
			.access_shared_handler(|s| s.ext.gui.get().copied());
		let Gui::Embedded { scale_factor, .. } = &mut self.gui else {
			panic!("called \"set_scale\" on a non-embedded gui")
		};

		if !API_TYPE.uses_logical_size()
			&& let Err(err) = ext
				.unwrap()
				.set_scale(&mut self.instance.plugin_handle(), scale.into())
		{
			// If I unwrap here, vital doesn't load. Why?
			warn!("{}: {err}", self.descriptor);
		} else {
			*scale_factor = scale;
		}
	}

	#[must_use]
	pub fn can_resize(&mut self) -> bool {
		let ext = self
			.instance
			.access_shared_handler(|s| s.ext.gui.get().copied());
		let Gui::Embedded { can_resize, .. } = &mut self.gui else {
			panic!("called \"can_resize\" on a non-embedded gui")
		};

		*can_resize
			.get_or_insert_with(|| ext.unwrap().can_resize(&mut self.instance.plugin_handle()))
	}

	pub fn call_on_main_thread_callback(&mut self) {
		self.instance.call_on_main_thread_callback();
	}

	pub fn activate(&mut self) {
		if self.instance.access_handler_mut(|mt| {
			let needs_param_rescan = mt.needs_param_rescan;
			mt.needs_param_rescan = false;
			needs_param_rescan
		}) {
			self.params = Param::all(&mut self.instance).unwrap_or_default();
		}

		let processor = self
			.instance
			.activate(|shared, _| AudioThread::new(shared), self.config)
			.unwrap()
			.into();

		if self
			.instance
			.access_shared_handler(|s| s.ext.latency.get().is_some())
		{
			self.latency_changed();
		}

		self.sender
			.push(AudioThreadMessage::Activated(NoDebug(processor)))
			.unwrap();
	}

	pub fn deactivate(&mut self, processor: NoClone<NoDebug<PluginAudioProcessor<Host>>>) {
		self.instance.deactivate(processor.0.0.into_stopped());
	}

	pub fn latency_changed(&mut self) {
		let latency = self
			.instance
			.access_shared_handler(|s| *s.ext.latency.get().unwrap());
		let latency = latency.get(&mut self.instance.plugin_handle());
		self.instance
			.access_shared_handler(|s| s.latency.store(latency, Relaxed));
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

	pub fn tick_timer(&mut self, timer_id: u32) {
		self.instance
			.access_shared_handler(|s| *s.ext.timer.get().unwrap())
			.on_timer(&mut self.instance.plugin_handle(), TimerId(timer_id));
	}

	pub fn create(&mut self) {
		self.destroy();

		let Some(&ext) = self.instance.access_shared_handler(|s| s.ext.gui.get()) else {
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
		let Gui::Embedded { .. } = self.gui else {
			panic!("called \"set_parent\" on a non-embedded gui");
		};

		// SAFETY:
		// Ensured by the caller.
		unsafe {
			self.instance
				.access_shared_handler(|s| *s.ext.gui.get().unwrap())
				.set_parent(
					&mut self.instance.plugin_handle(),
					ClapWindow::from_window_handle(window_handle).unwrap(),
				)
				.unwrap();
		}
	}

	pub fn show(&mut self) {
		if !self.is_open
			&& let Some(&ext) = self.instance.access_shared_handler(|s| s.ext.gui.get())
			&& let Err(err) = ext.show(&mut self.instance.plugin_handle())
		{
			// If I unwrap here, nih-plug plugins don't load. Why?
			warn!("{}: {err}", self.descriptor);
		}

		self.is_open = true;
	}

	pub fn destroy(&mut self) {
		if self.is_open
			&& let Some(&ext) = self.instance.access_shared_handler(|s| s.ext.gui.get())
		{
			ext.destroy(&mut self.instance.plugin_handle());
		}

		self.is_open = false;
	}

	#[must_use]
	pub fn get_size(&mut self) -> Option<Size> {
		let Gui::Embedded { scale_factor, .. } = self.gui else {
			return None;
		};

		let GuiSize { width, height } = self
			.instance
			.access_shared_handler(|s| *s.ext.gui.get().unwrap())
			.get_size(&mut self.instance.plugin_handle())?;
		Some(Size::from_native((width as f32, height as f32)).ensure_logical(scale_factor))
	}

	#[must_use]
	pub fn resize(&mut self, size: Size) -> Option<Size> {
		let Gui::Embedded {
			can_resize,
			scale_factor,
		} = self.gui
		else {
			return None;
		};

		if !can_resize.unwrap() {
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
		let size = ext
			.adjust_size(&mut self.instance.plugin_handle(), size)
			.unwrap();
		ext.set_size(&mut self.instance.plugin_handle(), size)
			.unwrap();

		let GuiSize { width, height } = size;
		Some(Size::from_native((width as f32, height as f32)).ensure_logical(scale_factor))
	}

	pub fn send_event(&mut self, event: Event) {
		self.sender.push(AudioThreadMessage::Event(event)).unwrap();
	}

	pub fn set_realtime(&mut self, realtime: bool) {
		if let Some(&render) = self.instance.access_shared_handler(|s| s.ext.render.get()) {
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
			.access_shared_handler(|s| s.ext.state.get().copied())?
			.save(&mut self.instance.plugin_handle(), &mut buf)
			.ok()?;

		Some(buf)
	}

	pub fn set_state(&mut self, buf: &[u8]) {
		self.instance
			.access_shared_handler(|s| s.ext.state.get().copied().unwrap())
			.load(&mut self.instance.plugin_handle(), &mut Cursor::new(buf))
			.unwrap();
	}
}

impl<Event: EventImpl> Drop for Plugin<Event> {
	fn drop(&mut self) {
		self.destroy();

		if matches!(
			self.instance.try_deactivate(),
			Err(PluginInstanceError::StillActivatedPlugin)
		) {
			warn!("leaked resources of {}", self.descriptor);
		}

		info!("dropped plugin {}", self.descriptor);
	}
}
