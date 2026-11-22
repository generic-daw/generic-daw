use crate::{
	ThreadPoolInjector,
	audio_buffers::AudioBuffers,
	event_buffers::EventBuffers,
	events::{EventImpl, TransportEvent},
	host::Host,
	param::Param,
	shared::CURRENT_THREAD_ID,
};
use clack_extensions::{params::ParamInfoFlags, tail::TailLength};
use clack_host::prelude::*;
use log::{info, warn};
use std::{collections::HashSet, sync::atomic::Ordering::Relaxed};
use utils::NoDebug;

#[derive(Debug)]
pub struct AudioThread {
	pub(crate) processor: NoDebug<PluginAudioProcessor<Host>>,
	audio_buffers: AudioBuffers,
	event_buffers: EventBuffers,
	param_count: usize,
	requires_process: HashSet<ClapId>,
	last_input: u64,
	last_output: u64,
	processing: bool,
}

impl AudioThread {
	#[must_use]
	pub fn new(
		processor: StoppedPluginAudioProcessor<Host>,
		audio_buffers: AudioBuffers,
		event_buffers: EventBuffers,
		params: &[Param],
	) -> Self {
		Self {
			processor: NoDebug(processor.into()),
			audio_buffers,
			event_buffers,
			param_count: params.len(),
			requires_process: params
				.iter()
				.filter_map(|param| {
					(param.flags.contains(ParamInfoFlags::REQUIRES_PROCESS)).then_some(param.id)
				})
				.collect(),
			last_input: 0,
			last_output: 0,
			processing: true,
		}
	}

	pub fn push_all(&mut self, events: impl IntoIterator<Item: EventImpl>) {
		self.event_buffers.push_all(events);
	}

	pub fn push(&mut self, event: impl EventImpl) {
		self.event_buffers.push(event);
	}

	#[must_use]
	pub fn config(&self) -> PluginAudioConfiguration {
		self.audio_buffers.config()
	}

	#[must_use]
	pub fn param_count(&self) -> usize {
		self.param_count
	}

	#[must_use]
	pub fn needs_restart(&self) -> bool {
		self.processor
			.access_shared_handler(|s| s.request_restart.load(Relaxed))
	}

	pub fn process<Event: EventImpl>(
		&mut self,
		audio: &mut [[f32; 2]],
		events: impl FnMut(Event),
		transport: Option<&TransportEvent>,
		injector: Option<ThreadPoolInjector<'_>>,
		mix_level: f32,
	) {
		let mut audio_in = None;
		macro_rules! audio_in {
			() => {
				*audio_in.get_or_insert_with(|| !self.audio_buffers.are_inputs_quiet(audio.len()))
			};
		}

		let mut events_in = None;
		macro_rules! events_in {
			() => {
				*events_in.get_or_insert_with(|| {
					self.event_buffers.inputs_require_process(|param_id| {
						self.requires_process.contains(&param_id)
					})
				})
			};
		}

		self.processor.access_shared_handler(|s| {
			CURRENT_THREAD_ID.with(|&id| s.audio_thread.store(id, Relaxed));
		});

		let steady_time = self.audio_buffers.read_in(audio);

		let request_process = self
			.processor
			.access_shared_handler(|s| s.request_process.swap(false, Relaxed));

		if !self.processing && !request_process && !events_in!() && !audio_in!() {
			self.flush(audio, events, mix_level);
			return;
		}

		let was_started = self.processor.is_started();

		let started_processor = match self.processor.ensure_processing_started() {
			Ok(started_processor) => started_processor,
			Err(err) => {
				warn!(
					"{}: {err}",
					self.processor.access_shared_handler(|s| &s.descriptor)
				);
				self.flush(audio, events, mix_level);
				return;
			}
		};

		if !was_started {
			info!(
				"{}: woke up from sleep",
				started_processor.access_shared_handler(|s| &s.descriptor)
			);
		}

		let (input_audio, mut output_audio) = self.audio_buffers.prepare(audio.len());
		let (input_events, mut output_events) = self.event_buffers.prepare();

		started_processor.access_handler_mut(|ap| {
			ap.injector = injector
				.map(|injector|
					// SAFETY:
					// `injector` is a valid reference to a `ThreadPoolInjector` at least
					// until this function returns, `ap.injector` is overwritten after the
					// call to `process`, and `AudioProcessor` never creates any copies of
					// `ap.injector` that may outlive `process`.
					unsafe { std::mem::transmute(injector) })
				.map(NoDebug);
		});

		let process_status = started_processor.process(
			&input_audio,
			&mut output_audio,
			&input_events,
			&mut output_events,
			Some(steady_time),
			transport,
		);

		self.processor.access_handler_mut(|ap| ap.injector = None);

		let process_status = match process_status {
			Ok(process_status) => process_status,
			Err(err) => {
				warn!(
					"{}: {err}",
					self.processor.access_shared_handler(|s| &s.descriptor)
				);
				self.flush(audio, events, mix_level);
				return;
			}
		};

		let steady_time = self.audio_buffers.write_out(audio, mix_level);
		self.event_buffers.write_out(events);

		if self
			.processor
			.access_shared_handler(|s| s.ext.tail.get().is_some())
			&& (request_process || events_in!() || audio_in!())
		{
			self.last_input = steady_time;
		}

		if !self.audio_buffers.are_outputs_quiet(audio.len()) {
			self.last_output = steady_time;
		}

		self.processing = match process_status {
			ProcessStatus::Continue => true,
			ProcessStatus::ContinueIfNotQuiet => {
				steady_time - self.last_output < self.audio_buffers.config().sample_rate as u64 / 20
			}
			ProcessStatus::Tail => self
				.processor
				.access_shared_handler(|s| s.ext.tail.get().copied())
				.is_some_and(|ext| match ext.get(&self.processor.plugin_handle()) {
					TailLength::Infinite => true,
					TailLength::Finite(tail) => steady_time - self.last_input < u64::from(tail),
				}),
			ProcessStatus::Sleep => false,
		};

		self.event_buffers.reset();

		self.processor
			.access_shared_handler(|s| s.request_flush.store(false, Relaxed));
	}

	fn flush<Event: EventImpl>(
		&mut self,
		audio: &mut [[f32; 2]],
		events: impl FnMut(Event),
		mix_level: f32,
	) {
		self.audio_buffers.flush(audio, mix_level);
		self.flush_events(events);
	}

	pub(crate) fn flush_events<Event: EventImpl>(&mut self, events: impl FnMut(Event)) {
		if self.processor.is_started() {
			info!(
				"{}: went to sleep",
				self.processor.access_shared_handler(|s| &s.descriptor)
			);
		}

		self.processor.ensure_processing_stopped();

		let events_in = !self.event_buffers.are_inputs_empty();
		let request_flush = self
			.processor
			.access_shared_handler(|s| s.request_flush.swap(false, Relaxed));

		if !request_flush && !events_in {
			return;
		}

		if let Some(&params) = self.processor.access_shared_handler(|s| s.ext.params.get()) {
			let (input_events, mut output_events) = self.event_buffers.prepare();

			params.flush_active(
				&mut self.processor.plugin_handle(),
				&input_events,
				&mut output_events,
			);

			self.event_buffers.write_out(events);
		}

		self.event_buffers.reset();
	}

	#[must_use]
	pub fn latency(&self) -> usize {
		self.audio_buffers.latency()
	}

	pub fn reset(&mut self) {
		self.processor.access_shared_handler(|s| {
			CURRENT_THREAD_ID.with(|&id| s.audio_thread.store(id, Relaxed));
		});
		self.processor.reset();
		self.audio_buffers.reset();
		self.event_buffers.reset();
		self.last_input = 0;
		self.last_output = 0;
		self.processing = true;
	}
}
