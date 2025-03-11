use crate::{shared::Shared, timer_ext::TimerExt};
use clack_extensions::{
    audio_ports::{HostAudioPortsImpl, RescanType},
    gui::{GuiSize, PluginGui},
    note_ports::{HostNotePortsImpl, NoteDialects, NotePortRescanFlags},
    timer::{HostTimerImpl, TimerId},
};
use clack_host::prelude::*;
use generic_daw_utils::NoDebug;
use std::{cell::RefCell, rc::Rc, time::Duration};

#[derive(Clone, Copy, Debug)]
pub enum MainThreadMessage {
    RequestCallback,
    GuiRequestHide,
    GuiRequestShow,
    GuiClosed,
    GuiRequestResize(GuiSize),
    TickTimers,
}

#[derive(Debug)]
pub struct MainThread<'a> {
    shared: &'a Shared,
    pub gui: Option<NoDebug<PluginGui>>,
    pub timers: Rc<RefCell<TimerExt>>,
}

impl<'a> MainThread<'a> {
    pub fn new(shared: &'a Shared) -> Self {
        Self {
            shared,
            gui: None,
            timers: Rc::default(),
        }
    }
}

impl<'a> MainThreadHandler<'a> for MainThread<'a> {
    fn initialized(&mut self, instance: InitializedPluginHandle<'_>) {
        self.gui = instance.get_extension().map(NoDebug);
        self.timers.borrow_mut().set_ext(instance.get_extension());
    }
}

impl HostAudioPortsImpl for MainThread<'_> {
    fn is_rescan_flag_supported(&self, _flag: RescanType) -> bool {
        false
    }

    fn rescan(&mut self, _flag: RescanType) {}
}

impl HostNotePortsImpl for MainThread<'_> {
    fn supported_dialects(&self) -> NoteDialects {
        NoteDialects::CLAP
    }

    fn rescan(&mut self, _flags: NotePortRescanFlags) {}
}

impl HostTimerImpl for MainThread<'_> {
    fn register_timer(&mut self, period_ms: u32) -> Result<TimerId, HostError> {
        let id = Ok(self
            .timers
            .borrow_mut()
            .register(Duration::from_millis(u64::from(period_ms))));

        self.shared
            .sender
            .try_send(MainThreadMessage::TickTimers)
            .unwrap();

        id
    }

    fn unregister_timer(&mut self, timer_id: TimerId) -> Result<(), HostError> {
        self.timers.borrow_mut().unregister(timer_id)
    }
}
