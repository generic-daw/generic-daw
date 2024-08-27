use super::timer::Timers;
use clack_extensions::{
    audio_ports::{HostAudioPortsImpl, RescanType},
    gui::PluginGui,
    note_ports::{HostNotePortsImpl, NoteDialects, NotePortRescanFlags},
    params::{HostParamsImplMainThread, ParamClearFlags, ParamRescanFlags},
    timer::{HostTimerImpl, PluginTimer, TimerId},
};
use clack_host::{
    host::{HostError, MainThreadHandler},
    plugin::InitializedPluginHandle,
    utils::ClapId,
};
use std::{rc::Rc, time::Duration};

pub struct HostPluginThread<'a> {
    plugin: Option<InitializedPluginHandle<'a>>,
    pub gui: Option<PluginGui>,
    timer_support: Option<PluginTimer>,
    timers: Rc<Timers>,
}

impl<'a> MainThreadHandler<'a> for HostPluginThread<'a> {
    fn initialized(&mut self, instance: InitializedPluginHandle<'a>) {
        self.gui = instance.get_extension();
        self.timer_support = instance.get_extension();
        self.plugin = Some(instance);
    }
}

impl<'a> HostAudioPortsImpl for HostPluginThread<'a> {
    fn is_rescan_flag_supported(&self, _flag: RescanType) -> bool {
        false
    }

    fn rescan(&mut self, _flag: RescanType) {}
}

impl<'a> HostNotePortsImpl for HostPluginThread<'a> {
    fn supported_dialects(&self) -> NoteDialects {
        NoteDialects::CLAP
    }

    fn rescan(&mut self, _flags: NotePortRescanFlags) {}
}

impl<'a> HostParamsImplMainThread for HostPluginThread<'a> {
    fn rescan(&mut self, _flags: ParamRescanFlags) {}

    fn clear(&mut self, _param_id: ClapId, _flags: ParamClearFlags) {}
}

impl<'a> HostPluginThread<'a> {
    pub fn new() -> Self {
        Self {
            plugin: None,
            gui: None,
            timer_support: None,
            timers: Rc::new(Timers::new()),
        }
    }
}

impl<'a> HostTimerImpl for HostPluginThread<'a> {
    fn register_timer(&mut self, period_ms: u32) -> Result<TimerId, HostError> {
        Ok(self
            .timers
            .register_new(Duration::from_millis(period_ms as u64)))
    }

    fn unregister_timer(&mut self, timer_id: TimerId) -> Result<(), HostError> {
        if self.timers.unregister(timer_id) {
            Ok(())
        } else {
            Err(HostError::Message("Unknown timer ID"))
        }
    }
}

impl<'a> Default for HostPluginThread<'a> {
    fn default() -> Self {
        Self::new()
    }
}
