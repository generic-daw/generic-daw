use crate::{Size, host::Host, shared::Shared};
use clack_extensions::{
	audio_ports::{HostAudioPortsImpl, RescanType},
	latency::HostLatencyImpl,
	note_ports::{HostNotePortsImpl, NoteDialects, NotePortRescanFlags},
	params::{HostParamsImplMainThread, ParamClearFlags, ParamRescanFlags},
	state::HostStateImpl,
	timer::{HostTimerImpl, TimerId},
};
use clack_host::{prelude::*, process::PluginAudioProcessor};
use generic_daw_utils::{NoClone, NoDebug};
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum MainThreadMessage {
	RequestCallback,
	LatencyChanged,
	Restart(NoClone<NoDebug<PluginAudioProcessor<Host>>>),
	Destroy(NoClone<NoDebug<PluginAudioProcessor<Host>>>),
	GuiRequestResize(Size),
	GuiRequestShow,
	GuiRequestHide,
	GuiClosed,
	RegisterTimer(u32, Duration),
	UnregisterTimer(u32),
	RescanParams(ParamRescanFlags),
	RescanParam(ClapId, ParamRescanFlags),
}

#[derive(Debug)]
pub struct MainThread<'a> {
	pub shared: &'a Shared<'a>,
	pub next_timer_id: u32,
	pub needs_param_rescan: bool,
}

impl<'a> MainThread<'a> {
	pub fn new(shared: &'a Shared<'a>) -> Self {
		Self {
			shared,
			next_timer_id: 0,
			needs_param_rescan: false,
		}
	}
}

impl<'a> MainThreadHandler<'a> for MainThread<'a> {
	fn initialized(&mut self, instance: InitializedPluginHandle<'a>) {
		self.shared.instance.set(instance).unwrap();
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
			.sender
			.send(MainThreadMessage::LatencyChanged)
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
		if flags.contains(ParamRescanFlags::ALL) {
			self.needs_param_rescan = true;
			self.shared.request_restart();
		} else if !flags.is_empty() {
			self.shared
				.sender
				.send(MainThreadMessage::RescanParams(flags))
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
			.sender
			.send(MainThreadMessage::RegisterTimer(
				timer_id.0,
				Duration::from_millis(period_ms.into()),
			))
			.unwrap();

		Ok(timer_id)
	}

	fn unregister_timer(&mut self, timer_id: TimerId) -> Result<(), HostError> {
		self.shared
			.sender
			.send(MainThreadMessage::UnregisterTimer(timer_id.0))
			.unwrap();

		Ok(())
	}
}
