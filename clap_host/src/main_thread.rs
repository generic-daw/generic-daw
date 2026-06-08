use crate::{AudioThread, Preset, Size, shared::Shared};
use clack_extensions::{
	audio_ports::{AudioPortRescanFlags, HostAudioPortsImpl},
	latency::HostLatencyImpl,
	note_ports::{HostNotePortsImpl, NoteDialects, NotePortRescanFlags},
	params::{HostParamsImplMainThread, ParamClearFlags, ParamRescanFlags},
	preset_discovery::{HostPresetLoadImpl, preset_data},
	state::HostStateImpl,
	timer::{HostTimerImpl, TimerId},
};
use clack_host::prelude::*;
use log::{log_enabled, warn};
use std::{ffi::CStr, fmt::Write as _, time::Duration};
use utils::NoClone;

#[derive(Clone, Debug)]
pub enum MainThreadMessage {
	RequestCallback,
	Restart(NoClone<AudioThread>),
	Deactivate(NoClone<AudioThread>),
	Destroy(NoClone<AudioThread>),
	GuiRequestResize(Size),
	GuiRequestShow,
	GuiRequestHide,
	GuiClosed,
	RegisterTimer(TimerId, Duration),
	UnregisterTimer(TimerId),
	RescanParams(ParamRescanFlags),
	PresetDiscovered(Preset),
	PresetLoaded(Preset),
}

#[derive(Debug)]
pub struct MainThread<'a> {
	pub shared: &'a Shared<'a>,
	pub params_rescan: bool,
	pub presets: Vec<Preset>,
	pub state: Option<Box<[u8]>>,
	pub next_timer_id: u32,
}

impl<'a> MainThread<'a> {
	pub fn new(shared: &'a Shared<'a>) -> Self {
		Self {
			shared,
			params_rescan: false,
			presets: Vec::new(),
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
	fn loaded(&mut self, location: preset_data::Location<'_>, load_key: Option<&CStr>) {
		if let Some(preset) = self.presets.iter().find(|preset| {
			preset.location.as_clap() == location && preset.load_key.as_deref() == load_key
		}) {
			self.shared
				.sender
				.send(MainThreadMessage::PresetLoaded(preset.clone()))
				.unwrap();
		}
	}

	#[expect(clippy::renamed_function_params)]
	fn on_error(
		&mut self,
		location: preset_data::Location<'_>,
		load_key: Option<&CStr>,
		error_code: i32,
		error_message: Option<&CStr>,
	) {
		if !log_enabled!(log::Level::Warn) {
			return;
		}

		let mut message = String::new();

		if let Some(preset) = self.presets.iter().find(|preset| {
			preset.location.as_clap() == location && preset.load_key.as_deref() == load_key
		}) {
			write!(message, "{}: {}", self.shared.descriptor, preset.name).unwrap();
		} else {
			write!(message, "{}: preset error", self.shared.descriptor).unwrap();
		}

		if let Some(error_message) = error_message {
			write!(message, ": {}", error_message.to_string_lossy()).unwrap();

			if error_code != 0 {
				write!(message, " (os error {error_code})").unwrap();
			}
		} else if error_code != 0 {
			write!(message, ": os error {error_code}").unwrap();
		}

		warn!("{message}");
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
