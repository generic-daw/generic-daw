use crate::{MainThreadMessage, PluginDescriptor, Size};
use async_channel::Sender;
use clack_extensions::{
	audio_ports::PluginAudioPorts,
	gui::{GuiSize, HostGuiImpl, PluginGui},
	latency::PluginLatency,
	log::{HostLogImpl, LogSeverity},
	note_ports::PluginNotePorts,
	params::{HostParamsImplShared, PluginParams},
	render::PluginRender,
	state::PluginState,
	thread_check::HostThreadCheckImpl,
	thread_pool::PluginThreadPool,
	timer::PluginTimer,
};
use clack_host::prelude::*;
use generic_daw_utils::NoDebug;
use log::{debug, error, info, warn};
use std::sync::{
	OnceLock,
	atomic::{AtomicU64, Ordering::Relaxed},
};

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(0);
thread_local! {
	pub static CURRENT_THREAD_ID: u64 = NEXT_THREAD_ID.fetch_add(1, Relaxed);
}

#[derive(Debug)]
pub struct Shared<'a> {
	pub descriptor: PluginDescriptor,
	pub sender: Sender<MainThreadMessage>,
	pub instance: OnceLock<InitializedPluginHandle<'a>>,
	pub audio_ports: OnceLock<NoDebug<PluginAudioPorts>>,
	pub gui: OnceLock<NoDebug<PluginGui>>,
	pub latency: OnceLock<NoDebug<PluginLatency>>,
	pub note_ports: OnceLock<NoDebug<PluginNotePorts>>,
	pub params: OnceLock<NoDebug<PluginParams>>,
	pub render: OnceLock<NoDebug<PluginRender>>,
	pub state: OnceLock<NoDebug<PluginState>>,
	pub thread_pool: OnceLock<NoDebug<PluginThreadPool>>,
	pub timer: OnceLock<NoDebug<PluginTimer>>,
	pub main_thread: u64,
	pub audio_thread: AtomicU64,
}

impl Shared<'_> {
	pub fn new(descriptor: PluginDescriptor, sender: Sender<MainThreadMessage>) -> Self {
		let main_thread = CURRENT_THREAD_ID.with(|id| *id);

		Self {
			descriptor,
			sender,
			instance: OnceLock::new(),
			audio_ports: OnceLock::new(),
			gui: OnceLock::new(),
			latency: OnceLock::new(),
			note_ports: OnceLock::new(),
			params: OnceLock::new(),
			render: OnceLock::new(),
			state: OnceLock::new(),
			thread_pool: OnceLock::new(),
			timer: OnceLock::new(),
			main_thread,
			audio_thread: AtomicU64::new(main_thread),
		}
	}
}

impl<'a> SharedHandler<'a> for Shared<'a> {
	fn initializing(&self, instance: InitializingPluginHandle<'a>) {
		macro_rules! initializing {
			($($ident:ident),*) => {
				$(
					if self.$ident.get().is_none()
						&& let Some(ext) = instance.get_extension()
					{
						_ = self.$ident.set(NoDebug(ext));
					}
				)*
			};
		}

		initializing![
			audio_ports,
			gui,
			latency,
			note_ports,
			params,
			render,
			state,
			thread_pool,
			timer
		];
	}

	fn request_process(&self) {}

	fn request_callback(&self) {
		self.sender
			.try_send(MainThreadMessage::RequestCallback)
			.unwrap();
	}

	fn request_restart(&self) {
		self.sender
			.try_send(MainThreadMessage::RequestRestart)
			.unwrap();
	}
}

impl HostGuiImpl for Shared<'_> {
	fn resize_hints_changed(&self) {}

	fn request_resize(&self, GuiSize { width, height }: GuiSize) -> Result<(), HostError> {
		self.sender
			.try_send(MainThreadMessage::GuiRequestResize(Size::from_native((
				width as f32,
				height as f32,
			))))
			.unwrap();

		Ok(())
	}

	fn request_show(&self) -> Result<(), HostError> {
		self.sender
			.try_send(MainThreadMessage::GuiRequestShow)
			.unwrap();

		Ok(())
	}

	fn request_hide(&self) -> Result<(), HostError> {
		self.sender
			.try_send(MainThreadMessage::GuiRequestHide)
			.unwrap();

		Ok(())
	}

	fn closed(&self, _was_destroyed: bool) {
		self.sender.try_send(MainThreadMessage::GuiClosed).unwrap();
	}
}

impl HostLogImpl for Shared<'_> {
	fn log(&self, severity: LogSeverity, message: &str) {
		match severity {
			LogSeverity::Debug => debug!("{}: {message}", self.descriptor),
			LogSeverity::Info => info!("{}: {message}", self.descriptor),
			LogSeverity::Warning => warn!("{}: {message}", self.descriptor),
			LogSeverity::Error
			| LogSeverity::Fatal
			| LogSeverity::HostMisbehaving
			| LogSeverity::PluginMisbehaving => error!("{}: {message}", self.descriptor),
		}
	}
}

impl HostParamsImplShared for Shared<'_> {
	fn request_flush(&self) {}
}

impl HostThreadCheckImpl for Shared<'_> {
	fn is_main_thread(&self) -> bool {
		CURRENT_THREAD_ID.with(|&id| id == self.main_thread)
	}

	fn is_audio_thread(&self) -> bool {
		CURRENT_THREAD_ID.with(|&id| id == self.audio_thread.load(Relaxed))
	}
}
