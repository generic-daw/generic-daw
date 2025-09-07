use crate::shared::Shared;
use clack_extensions::thread_pool::{HostThreadPoolImpl, PluginThreadPool};
use clack_host::prelude::*;

#[derive(Debug)]
pub struct AudioThread<'a> {
	shared: &'a Shared<'a>,
}

impl<'a> AudioThread<'a> {
	pub fn new(shared: &'a Shared<'a>) -> Self {
		Self { shared }
	}
}

impl<'a> AudioProcessorHandler<'a> for AudioThread<'a> {}

impl HostThreadPoolImpl for AudioThread<'_> {
	fn request_exec(&mut self, task_count: u32) -> Result<(), HostError> {
		let instance = self.shared.instance.get().unwrap();
		let ext = instance.get_extension::<PluginThreadPool>().unwrap();

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
