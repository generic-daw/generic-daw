use crate::{shared::Shared, timer::Timers};
use clack_extensions::{
    audio_ports::{HostAudioPortsImpl, RescanType},
    gui::{GuiSize, PluginGui},
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

pub struct MainThread<'a> {
    shared: &'a Shared,
    pub gui: Option<PluginGui>,
    pub timer_support: Option<PluginTimer>,
    pub timers: Rc<RefCell<Timers>>,
}

impl<'a> MainThread<'a> {
    pub fn new(shared: &'a Shared) -> Self {
        Self {
            shared,
            gui: None,
            timer_support: None,
            timers: Rc::default(),
        }
    }
}

impl<'a> MainThreadHandler<'a> for MainThread<'a> {
    fn initialized(&mut self, instance: InitializedPluginHandle<'_>) {
        self.gui = instance.get_extension();
        self.timer_support = instance.get_extension();
    }
}

impl HostAudioPortsImpl for MainThread<'_> {
    fn is_rescan_flag_supported(&self, _flag: RescanType) -> bool {
        false
    }

    fn rescan(&mut self, _flag: RescanType) {}
}

impl HostTimerImpl for MainThread<'_> {
    fn register_timer(&mut self, period_ms: u32) -> Result<TimerId, HostError> {
        let id = Ok(self
            .timers
            .borrow_mut()
            .register(Duration::from_millis(u64::from(period_ms))));

        self.shared.sender.try_send(GuiMessage::TickTimers).unwrap();

        id
    }

    fn unregister_timer(&mut self, timer_id: TimerId) -> Result<(), HostError> {
        self.timers.borrow_mut().unregister(timer_id)
    }
}
