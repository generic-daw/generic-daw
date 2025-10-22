use crate::shared::Shared;
use clack_extensions::thread_pool::HostThreadPoolImpl;
use clack_host::prelude::*;
use std::sync::atomic::Ordering::Relaxed;

#[derive(Debug)]
pub struct AudioThread<'a> {
	shared: &'a Shared<'a>,
	pub processing: bool,
}

impl<'a> AudioThread<'a> {
	pub fn new(shared: &'a Shared<'a>) -> Self {
		shared.needs_restart.store(false, Relaxed);

		Self {
			shared,
			processing: false,
		}
	}
}

impl<'a> AudioProcessorHandler<'a> for AudioThread<'a> {}

impl HostThreadPoolImpl for AudioThread<'_> {
	fn request_exec(&mut self, task_count: u32) -> Result<(), HostError> {
		if !self.processing {
			return Err(HostError::Message(
				"called `request_exec` while not processing",
			));
		}

		if task_count == 0 {
			return Ok(());
		}

		let instance = self.shared.instance.get().unwrap();
		let ext = self.shared.ext.thread_pool.get().unwrap();

		rayon_core::in_place_scope(|s| {
			for i in 1..task_count {
				s.spawn(move |_| {
					instance.access(|s| ext.exec(&s, i)).unwrap();
				});
			}

			instance.access(|s| ext.exec(&s, 0)).unwrap();
		});

		Ok(())
	}
}
