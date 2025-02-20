use crate::timer::Timers;
use clack_extensions::{
    audio_ports::{HostAudioPortsImpl, RescanType},
    gui::{GuiSize, PluginGui},
    note_ports::{HostNotePortsImpl, NoteDialects, NotePortRescanFlags},
    params::{HostParamsImplMainThread, ParamClearFlags, ParamRescanFlags},
    state::HostStateImpl,
    timer::{HostTimerImpl, PluginTimer, TimerId},
};
use clack_host::prelude::*;
use std::{cell::RefCell, rc::Rc, time::Duration};

#[derive(Clone, Copy, Debug)]
pub enum GuiMessage {
    RequestCallback,
    GuiRequestHide,
    GuiRequestShow,
    GuiClosed,
    GuiRequestResize(GuiSize),
    TickTimers,
}

#[derive(Default)]
pub struct MainThread {
    pub gui: Option<PluginGui>,
    pub timer_support: Option<PluginTimer>,
    pub timers: Rc<RefCell<Timers>>,
}

impl MainThreadHandler<'_> for MainThread {
    fn initialized(&mut self, instance: InitializedPluginHandle<'_>) {
        self.gui = instance.get_extension();
        self.timer_support = instance.get_extension();
    }
}

impl HostAudioPortsImpl for MainThread {
    fn is_rescan_flag_supported(&self, _flag: RescanType) -> bool {
        false
    }

    fn rescan(&mut self, _flag: RescanType) {}
}

impl HostNotePortsImpl for MainThread {
    fn supported_dialects(&self) -> NoteDialects {
        NoteDialects::CLAP
    }

    fn rescan(&mut self, _flags: NotePortRescanFlags) {}
}

impl HostParamsImplMainThread for MainThread {
    fn clear(&mut self, _id: ClapId, _flags: ParamClearFlags) {}

    fn rescan(&mut self, _flags: ParamRescanFlags) {}
}

impl HostStateImpl for MainThread {
    fn mark_dirty(&mut self) {}
}

impl HostTimerImpl for MainThread {
    fn register_timer(&mut self, period_ms: u32) -> Result<TimerId, HostError> {
        Ok(self
            .timers
            .borrow_mut()
            .register(Duration::from_millis(u64::from(period_ms))))
    }

    fn unregister_timer(&mut self, timer_id: TimerId) -> Result<(), HostError> {
        self.timers.borrow_mut().unregister(timer_id)
    }
}
