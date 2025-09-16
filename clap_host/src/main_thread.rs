use crate::{Size, audio_processor_wrapper::AudioProcessorWrapper, shared::Shared};
use clack_extensions::{
	audio_ports::{HostAudioPortsImpl, RescanType},
	latency::HostLatencyImpl,
	note_ports::{HostNotePortsImpl, NoteDialects, NotePortRescanFlags},
	params::{HostParamsImplMainThread, ParamClearFlags, ParamRescanFlags},
	state::HostStateImpl,
	timer::{HostTimerImpl, TimerId},
};
use clack_host::prelude::*;
use generic_daw_utils::NoClone;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum MainThreadMessage {
	RequestCallback,
	RequestRestart,
	Restart(NoClone<AudioProcessorWrapper>),
	GuiRequestShow,
	GuiRequestResize(Size),
	GuiRequestHide,
	GuiClosed,
	RegisterTimer(u32, Duration),
	UnregisterTimer(u32),
	RescanParamValues,
	ParamUpdate(ClapId, f32),
}

#[derive(Debug)]
pub struct MainThread<'a> {
	shared: &'a Shared<'a>,
	next_timer_id: u32,
}

impl<'a> MainThread<'a> {
	pub fn new(shared: &'a Shared<'a>) -> Self {
		Self {
			shared,
			next_timer_id: 0,
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
		if flags.contains(ParamRescanFlags::VALUES) {
			self.shared
				.sender
				.try_send(MainThreadMessage::RescanParamValues)
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
			.try_send(MainThreadMessage::RegisterTimer(
				timer_id.0,
				Duration::from_millis(period_ms.into()),
			))
			.unwrap();

		Ok(timer_id)
	}

	fn unregister_timer(&mut self, timer_id: TimerId) -> Result<(), HostError> {
		self.shared
			.sender
			.try_send(MainThreadMessage::UnregisterTimer(timer_id.0))
			.unwrap();

		Ok(())
	}
}
