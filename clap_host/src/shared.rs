use crate::{MainThreadMessage, PluginDescriptor, Size};
use async_channel::Sender;
use clack_extensions::{
	gui::{GuiSize, HostGuiImpl},
	log::{HostLogImpl, LogSeverity},
	params::HostParamsImplShared,
};
use clack_host::prelude::*;
use log::{debug, error, info, warn};

#[derive(Debug)]
pub struct Shared {
	pub descriptor: PluginDescriptor,
	pub sender: Sender<MainThreadMessage>,
}

impl Shared {
	pub fn new(descriptor: PluginDescriptor, sender: Sender<MainThreadMessage>) -> Self {
		Self { descriptor, sender }
	}
}

impl SharedHandler<'_> for Shared {
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

impl HostGuiImpl for Shared {
	fn resize_hints_changed(&self) {}

	fn request_resize(&self, GuiSize { width, height }: GuiSize) -> Result<(), HostError> {
		self.sender
			.try_send(MainThreadMessage::GuiRequestResize(Size::Native {
				width: width as f32,
				height: height as f32,
			}))
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

impl HostLogImpl for Shared {
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

impl HostParamsImplShared for Shared {
	fn request_flush(&self) {}
}
