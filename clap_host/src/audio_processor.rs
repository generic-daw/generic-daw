use crate::{audio_buffers::AudioBuffers, event_buffers::EventBuffers, shared::Shared};
use clack_extensions::{
	tail::HostTailImpl,
	thread_pool::{HostThreadPoolImpl, PluginThreadPool},
};
use clack_host::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use utils::NoDebug;

#[derive(Debug)]
pub struct ThreadPoolExecutor<'a> {
	shared: PluginSharedHandle<'a>,
	thread_pool: NoDebug<PluginThreadPool>,
	task_count: u32,
	task_index: AtomicU32,
}

impl ThreadPoolExecutor<'_> {
	pub fn task_count(&self) -> u32 {
		self.task_count
	}

	pub fn next_task(&self) -> Option<u32> {
		let task_index = self.task_index.fetch_add(1, Relaxed);
		(task_index < self.task_count).then_some(task_index)
	}

	pub fn exec_task(&self, task_index: u32) {
		debug_assert!(task_index < self.task_count);
		self.thread_pool.exec(&self.shared, task_index);
	}
}

pub type ThreadPoolInjector<'a> = &'a mut (dyn for<'b> FnMut(ThreadPoolExecutor<'b>) + Send + 'a);

#[derive(Debug)]
pub struct AudioProcessor<'a> {
	pub shared: &'a Shared<'a>,
	pub audio_buffers: Option<AudioBuffers>,
	pub event_buffers: Option<EventBuffers>,
	pub processing: bool,
	pub last_input: Option<u64>,
	pub injector: Option<NoDebug<ThreadPoolInjector<'a>>>,
}

impl<'a> AudioProcessor<'a> {
	pub fn new(
		shared: &'a Shared<'a>,
		audio_buffers: AudioBuffers,
		event_buffers: EventBuffers,
	) -> Self {
		shared.request_restart.store(false, Relaxed);

		Self {
			shared,
			audio_buffers: Some(audio_buffers),
			event_buffers: Some(event_buffers),
			processing: false,
			last_input: None,
			injector: None,
		}
	}
}

impl<'a> AudioProcessorHandler<'a> for AudioProcessor<'a> {}

impl HostTailImpl for AudioProcessor<'_> {
	fn changed(&mut self) {}
}

impl HostThreadPoolImpl for AudioProcessor<'_> {
	fn request_exec(&mut self, task_count: u32) -> Result<(), HostError> {
		let Some(mut injector) = self.injector.take() else {
			return Err(HostError::Message("no thread pool injector available"));
		};

		self.shared.instance.get().unwrap().access(|shared| {
			injector(ThreadPoolExecutor {
				shared,
				thread_pool: *self.shared.ext.thread_pool.get().unwrap(),
				task_count,
				task_index: AtomicU32::new(0),
			});
		});

		self.injector = Some(injector);

		Ok(())
	}
}

impl Drop for AudioProcessor<'_> {
	fn drop(&mut self) {
		self.shared.request_deactivate.store(false, Relaxed);
	}
}
