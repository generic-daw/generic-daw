use crate::{shared::Shared, timer_ext::TimerExt};
use clack_extensions::{
    audio_ports::{HostAudioPortsImpl, RescanType},
    gui::{GuiSize, PluginGui},
    latency::{HostLatencyImpl, PluginLatency},
    note_ports::{HostNotePortsImpl, NoteDialects, NotePortRescanFlags},
    timer::{HostTimerImpl, TimerId},
};
use clack_host::prelude::*;
use generic_daw_utils::NoDebug;
use log::info;
use std::{cell::RefCell, rc::Rc, time::Duration};

#[derive(Clone, Copy, Debug)]
pub enum MainThreadMessage {
    RequestCallback,
    GuiRequestShow,
    GuiRequestResize(GuiSize),
    GuiRequestHide,
    GuiClosed,
    TickTimers,
    LatencyChanged,
}

#[derive(Debug)]
pub struct MainThread<'a> {
    shared: &'a Shared,
    pub gui: Option<NoDebug<PluginGui>>,
    pub latency: Option<NoDebug<PluginLatency>>,
    pub timers: Rc<RefCell<TimerExt>>,
}

impl<'a> MainThread<'a> {
    pub fn new(shared: &'a Shared) -> Self {
        Self {
            shared,
            gui: None,
            timers: Rc::default(),
            latency: None,
        }
    }
}

impl<'a> MainThreadHandler<'a> for MainThread<'a> {
    fn initialized(&mut self, instance: InitializedPluginHandle<'_>) {
        self.gui = instance.get_extension().map(NoDebug);
        self.latency = instance.get_extension().map(NoDebug);
        self.timers.borrow_mut().set_ext(instance.get_extension());
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

impl HostTimerImpl for MainThread<'_> {
    fn register_timer(&mut self, period_ms: u32) -> Result<TimerId, HostError> {
        let timer_id = self
            .timers
            .borrow_mut()
            .register(Duration::from_millis(u64::from(period_ms)));

        info!(
            "{} ({}): registered {period_ms}ms timer with id {timer_id}",
            self.shared.descriptor.name, self.shared.descriptor.id
        );

        self.shared
            .main_sender
            .try_send(MainThreadMessage::TickTimers)
            .unwrap();

        Ok(timer_id)
    }

    fn unregister_timer(&mut self, timer_id: TimerId) -> Result<(), HostError> {
        info!(
            "{} ({}): unregistered timer with id {timer_id}",
            self.shared.descriptor.name, self.shared.descriptor.id
        );

        self.timers.borrow_mut().unregister(timer_id)
    }
}
