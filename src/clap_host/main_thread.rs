use super::{shared::Shared, timer::Timers};
use clack_extensions::{
    audio_ports::{HostAudioPortsImpl, RescanType},
    gui::{GuiSize, PluginGui},
    note_ports::{HostNotePortsImpl, NoteDialects, NotePortRescanFlags},
    params::{HostParamsImplMainThread, ParamClearFlags, ParamRescanFlags},
    state::HostStateImpl,
    timer::{HostTimerImpl, PluginTimer, TimerId},
};
use clack_host::prelude::*;
use std::{rc::Rc, time::Duration};

pub enum MainThreadMessage {
    RunOnMainThread,
    GuiClosed,
    GuiRequestResized(GuiSize),
    ProcessAudio(Vec<Vec<f32>>, AudioPorts, AudioPorts, EventBuffer),
    GetCounter,
    GetState,
    SetState(Vec<u8>),
}

pub struct MainThread<'a> {
    pub shared: &'a Shared,
    plugin: Option<InitializedPluginHandle<'a>>,
    pub gui: Option<PluginGui>,
    pub timer_support: Option<PluginTimer>,
    pub timers: Rc<Timers>,
    pub dirty: bool,
}

impl<'a> MainThread<'a> {
    pub fn new(shared: &'a Shared) -> Self {
        Self {
            shared,
            plugin: None,
            gui: None,
            timer_support: None,
            timers: Rc::default(),
            dirty: false,
        }
    }
}

impl<'a> MainThreadHandler<'a> for MainThread<'a> {
    fn initialized(&mut self, instance: InitializedPluginHandle<'a>) {
        self.gui = instance.get_extension();
        self.timer_support = instance.get_extension();
        self.timers = Rc::new(Timers::default());
        self.plugin = Some(instance);
    }
}

impl HostAudioPortsImpl for MainThread<'_> {
    fn is_rescan_flag_supported(&self, _flag: RescanType) -> bool {
        false
    }

    fn rescan(&mut self, _flag: RescanType) {
        // we don't support audio ports changing on the fly (yet)
    }
}

impl HostNotePortsImpl for MainThread<'_> {
    fn supported_dialects(&self) -> NoteDialects {
        NoteDialects::CLAP
    }

    fn rescan(&mut self, _flags: NotePortRescanFlags) {
        // We don't support note ports changing on the fly (yet)
    }
}

impl HostParamsImplMainThread for MainThread<'_> {
    fn clear(&mut self, _id: ClapId, _flags: ParamClearFlags) {}

    fn rescan(&mut self, _flags: ParamRescanFlags) {
        // We don't track param values (yet)
    }
}

impl HostStateImpl for MainThread<'_> {
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

impl HostTimerImpl for MainThread<'_> {
    fn register_timer(&mut self, period_ms: u32) -> Result<TimerId, HostError> {
        Ok(self
            .timers
            .register_new(Duration::from_millis(u64::from(period_ms))))
    }

    fn unregister_timer(&mut self, timer_id: TimerId) -> Result<(), HostError> {
        if self.timers.unregister(timer_id) {
            Ok(())
        } else {
            Err(HostError::Message("Unknown timer ID"))
        }
    }
}
