use crate::{EventImpl, Size, shared::Shared};
use clack_extensions::{
	audio_ports::{HostAudioPortsImpl, RescanType},
	gui::PluginGui,
	latency::{HostLatencyImpl, PluginLatency},
	note_ports::{HostNotePortsImpl, NoteDialects, NotePortRescanFlags},
	params::{HostParamsImplMainThread, ParamClearFlags, ParamRescanFlags, PluginParams},
	render::PluginRender,
	state::{HostStateImpl, PluginState},
	timer::{HostTimerImpl, PluginTimer, TimerId},
};
use clack_host::prelude::*;
use generic_daw_utils::NoDebug;
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub enum MainThreadMessage<Event: EventImpl> {
	RequestCallback,
	GuiRequestShow,
	GuiRequestResize(Size),
	GuiRequestHide,
	GuiClosed,
	RegisterTimer(u32, Duration),
	UnregisterTimer(u32),
	LatencyChanged,
	RescanValues,
	LiveEvent(Event),
}

#[derive(Debug)]
pub struct MainThread<'a, Event: EventImpl> {
	shared: &'a Shared<Event>,
	pub gui: Option<NoDebug<PluginGui>>,
	pub latency: Option<NoDebug<PluginLatency>>,
	pub params: Option<NoDebug<PluginParams>>,
	pub render: Option<NoDebug<PluginRender>>,
	pub state: Option<NoDebug<PluginState>>,
	pub timers: Option<NoDebug<PluginTimer>>,
	next_timer_id: u32,
}

impl<'a, Event: EventImpl> MainThread<'a, Event> {
	pub fn new(shared: &'a Shared<Event>) -> Self {
		Self {
			shared,
			gui: None,
			timers: None,
			params: None,
			render: None,
			state: None,
			latency: None,
			next_timer_id: 0,
		}
	}
}

impl<'a, Event: EventImpl> MainThreadHandler<'a> for MainThread<'a, Event> {
	fn initialized(&mut self, instance: InitializedPluginHandle<'_>) {
		self.gui = instance.get_extension().map(NoDebug);
		self.timers = instance.get_extension().map(NoDebug);
		self.params = instance.get_extension().map(NoDebug);
		self.render = instance.get_extension().map(NoDebug);
		self.state = instance.get_extension().map(NoDebug);
		self.latency = instance.get_extension().map(NoDebug);
	}
}

impl<Event: EventImpl> HostAudioPortsImpl for MainThread<'_, Event> {
	fn is_rescan_flag_supported(&self, _flag: RescanType) -> bool {
		false
	}

	fn rescan(&mut self, _flag: RescanType) {}
}

impl<Event: EventImpl> HostLatencyImpl for MainThread<'_, Event> {
	fn changed(&mut self) {
		self.shared
			.main_sender
			.try_send(MainThreadMessage::LatencyChanged)
			.unwrap();
	}
}

impl<Event: EventImpl> HostNotePortsImpl for MainThread<'_, Event> {
	fn supported_dialects(&self) -> NoteDialects {
		NoteDialects::CLAP | NoteDialects::MIDI
	}

	fn rescan(&mut self, _flags: NotePortRescanFlags) {}
}

impl<Event: EventImpl> HostParamsImplMainThread for MainThread<'_, Event> {
	fn rescan(&mut self, flags: ParamRescanFlags) {
		if flags.contains(ParamRescanFlags::VALUES) {
			self.shared
				.main_sender
				.try_send(MainThreadMessage::RescanValues)
				.unwrap();
		}
	}

	fn clear(&mut self, _param_id: ClapId, _flags: ParamClearFlags) {}
}

impl<Event: EventImpl> HostStateImpl for MainThread<'_, Event> {
	fn mark_dirty(&mut self) {}
}

impl<Event: EventImpl> HostTimerImpl for MainThread<'_, Event> {
	fn register_timer(&mut self, period_ms: u32) -> Result<TimerId, HostError> {
		let timer_id = TimerId(self.next_timer_id);
		self.next_timer_id += 1;

		self.shared
			.main_sender
			.try_send(MainThreadMessage::RegisterTimer(
				timer_id.0,
				Duration::from_millis(period_ms.into()),
			))
			.unwrap();

		Ok(timer_id)
	}

	fn unregister_timer(&mut self, timer_id: TimerId) -> Result<(), HostError> {
		self.shared
			.main_sender
			.try_send(MainThreadMessage::UnregisterTimer(timer_id.0))
			.unwrap();

		Ok(())
	}
}
