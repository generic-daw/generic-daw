use crate::shared::Shared;
use clack_extensions::thread_pool::HostThreadPoolImpl;
use clack_host::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering::Acquire};

#[derive(Debug)]
pub struct AudioThread<'a> {
	shared: &'a Shared<'a>,
	pub processing: AtomicBool,
}

impl<'a> AudioThread<'a> {
	pub fn new(shared: &'a Shared<'a>) -> Self {
		Self {
			shared,
			processing: AtomicBool::new(false),
		}
	}
}

impl<'a> AudioProcessorHandler<'a> for AudioThread<'a> {}

impl HostThreadPoolImpl for AudioThread<'_> {
	fn request_exec(&mut self, task_count: u32) -> Result<(), HostError> {
		if !self.processing.load(Acquire) {
			return Err(HostError::Message(
				"called `request_exec` while outside the `process` call",
			));
		}

		let instance = self.shared.instance.get().unwrap();
		let ext = self.shared.thread_pool.get().unwrap();

		match task_count {
			0 => {}
			1 => instance.access(|s| ext.exec(&s, 0)).unwrap(),
			_ => {
				rayon_core::scope(|s| {
					for i in 0..task_count {
						s.spawn(move |_| {
							instance.access(|s| ext.exec(&s, i)).unwrap();
						});
					}
				});
			}
		}

		Ok(())
	}
}
