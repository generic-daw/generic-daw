use crate::{Size, host::Host, preset::Preset, shared::Shared};
use clack_extensions::{
	audio_ports::{AudioPortRescanFlags, HostAudioPortsImpl},
	latency::HostLatencyImpl,
	note_ports::{HostNotePortsImpl, NoteDialects, NotePortRescanFlags},
	params::{HostParamsImplMainThread, ParamClearFlags, ParamRescanFlags},
	preset_discovery::{HostPresetLoadImpl, prelude::*},
	state::HostStateImpl,
	timer::{HostTimerImpl, TimerId},
};
use clack_host::prelude::*;
use std::{ffi::CStr, time::Duration};
use utils::{NoClone, NoDebug};

#[derive(Clone, Debug)]
pub enum MainThreadMessage {
	RequestCallback,
	Restart(NoClone<NoDebug<StoppedPluginAudioProcessor<Host>>>),
	Deactivate(NoClone<NoDebug<StoppedPluginAudioProcessor<Host>>>),
	Destroy(NoClone<NoDebug<StoppedPluginAudioProcessor<Host>>>),
	GuiRequestResize(Size),
	GuiRequestShow,
	GuiRequestHide,
	GuiClosed,
	RegisterTimer(TimerId, Duration),
	UnregisterTimer(TimerId),
	RescanParams(ParamRescanFlags),
	PresetDiscovered(Preset),
}

#[derive(Debug)]
pub struct MainThread<'a> {
	pub shared: &'a Shared<'a>,
	pub params_rescan: bool,
	pub state: Option<Box<[u8]>>,
	pub next_timer_id: u32,
}

impl<'a> MainThread<'a> {
	pub fn new(shared: &'a Shared<'a>) -> Self {
		Self {
			shared,
			params_rescan: false,
			state: None,
			next_timer_id: 0,
		}
	}
}

impl<'a> MainThreadHandler<'a> for MainThread<'a> {
	fn initialized(&mut self, instance: InitializedPluginHandle<'a>) {
		self.shared.initialized(instance);
	}
}

impl HostAudioPortsImpl for MainThread<'_> {
	fn is_rescan_flag_supported(&self, _flag: AudioPortRescanFlags) -> bool {
		true
	}

	fn rescan(&mut self, _flags: AudioPortRescanFlags) {}
}

impl HostLatencyImpl for MainThread<'_> {
	fn changed(&mut self) {}
}

impl HostNotePortsImpl for MainThread<'_> {
	fn supported_dialects(&self) -> NoteDialects {
		NoteDialects::CLAP | NoteDialects::MIDI
	}

	fn rescan(&mut self, _flags: NotePortRescanFlags) {}
}

impl HostParamsImplMainThread for MainThread<'_> {
	fn rescan(&mut self, flags: ParamRescanFlags) {
		if flags.requires_restart() {
			self.params_rescan = true;
		} else if !flags.is_empty() {
			self.shared
				.sender
				.send(MainThreadMessage::RescanParams(flags))
				.unwrap();
		}
	}

	fn clear(&mut self, _param_id: ClapId, _flags: ParamClearFlags) {}
}

impl HostPresetLoadImpl for MainThread<'_> {
	fn loaded(&mut self, _location: Location<'_>, _load_key: Option<&CStr>) {}

	fn on_error(
		&mut self,
		_location: Location<'_>,
		_load_key: Option<&CStr>,
		_os_error: i32,
		_message: Option<&CStr>,
	) {
	}
}

impl HostStateImpl for MainThread<'_> {
	fn mark_dirty(&mut self) {
		self.state = None;
	}
}

impl HostTimerImpl for MainThread<'_> {
	fn register_timer(&mut self, period_ms: u32) -> Result<TimerId, HostError> {
		let timer_id = TimerId(self.next_timer_id);
		self.next_timer_id += 1;

		self.shared
			.sender
			.send(MainThreadMessage::RegisterTimer(
				timer_id,
				Duration::from_millis(period_ms.into()),
			))
			.unwrap();

		Ok(timer_id)
	}

	fn unregister_timer(&mut self, timer_id: TimerId) -> Result<(), HostError> {
		self.shared
			.sender
			.send(MainThreadMessage::UnregisterTimer(timer_id))
			.unwrap();

		Ok(())
	}
}
