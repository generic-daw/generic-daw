use crate::shared::Shared;
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
pub enum MainThreadMessage {
	RequestCallback,
	GuiRequestShow,
	GuiRequestResize([f32; 2]),
	GuiRequestHide,
	GuiClosed,
	RegisterTimer(u32, Duration),
	UnregisterTimer(u32),
	LatencyChanged,
	ParamChanged(ClapId, f32),
	RescanValues,
}

#[derive(Debug)]
pub struct MainThread<'a> {
	shared: &'a Shared,
	pub gui: Option<NoDebug<PluginGui>>,
	pub latency: Option<NoDebug<PluginLatency>>,
	pub params: Option<NoDebug<PluginParams>>,
	pub render: Option<NoDebug<PluginRender>>,
	pub state: Option<NoDebug<PluginState>>,
	pub timers: Option<NoDebug<PluginTimer>>,
	next_timer_id: u32,
}

impl<'a> MainThread<'a> {
	pub fn new(shared: &'a Shared) -> Self {
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

impl<'a> MainThreadHandler<'a> for MainThread<'a> {
	fn initialized(&mut self, instance: InitializedPluginHandle<'_>) {
		self.gui = instance.get_extension().map(NoDebug);
		self.timers = instance.get_extension().map(NoDebug);
		self.params = instance.get_extension().map(NoDebug);
		self.render = instance.get_extension().map(NoDebug);
		self.state = instance.get_extension().map(NoDebug);
		self.latency = instance.get_extension().map(NoDebug);
	}
}

impl HostAudioPortsImpl for MainThread<'_> {
	fn is_rescan_flag_supported(&self, _flag: RescanType) -> bool {
		false
	}

	fn rescan(&mut self, _flag: RescanType) {}
}

impl HostLatencyImpl for MainThread<'_> {
	fn changed(&mut self) {
		self.shared
			.main_sender
			.try_send(MainThreadMessage::LatencyChanged)
			.unwrap();
	}
}

impl HostNotePortsImpl for MainThread<'_> {
	fn supported_dialects(&self) -> NoteDialects {
		NoteDialects::CLAP | NoteDialects::MIDI
	}

	fn rescan(&mut self, _flags: NotePortRescanFlags) {}
}

impl HostParamsImplMainThread for MainThread<'_> {
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

impl HostStateImpl for MainThread<'_> {
	fn mark_dirty(&mut self) {}
}

impl HostTimerImpl for MainThread<'_> {
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
