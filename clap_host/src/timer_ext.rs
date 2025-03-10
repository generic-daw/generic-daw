use clack_extensions::timer::{PluginTimer, TimerId};
use clack_host::prelude::*;
use generic_daw_utils::{HoleyVec, NoDebug};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct TimerExt {
    ext: NoDebug<PluginTimer>,
    timers: HoleyVec<(Duration, Instant)>,
    next_id: usize,
}

impl TimerExt {
    pub fn new(ext: PluginTimer) -> Self {
        Self {
            ext: ext.into(),
            timers: HoleyVec::default(),
            next_id: 0,
        }
    }

    pub fn tick_timers(&mut self, plugin: &mut PluginMainThreadHandle<'_>) -> Option<Duration> {
        let now = Instant::now();
        let mut sleep = None;

        for (id, (interval, tick)) in self.timers.iter_mut() {
            if *tick <= now {
                self.ext.on_timer(plugin, TimerId(id as u32));
                *tick = now + *interval;
            }

            if sleep.is_none_or(|next| next > *tick) {
                sleep = Some(*tick);
            }
        }

        sleep.map(|next| next - now)
    }

    pub fn register(&mut self, interval: Duration) -> TimerId {
        let id = TimerId(self.next_id as u32);

        self.timers.insert(self.next_id, (interval, Instant::now()));
        self.next_id += 1;

        id
    }

    pub fn unregister(&mut self, id: TimerId) -> Result<(), HostError> {
        self.timers
            .remove(id.0 as usize)
            .map(|_| ())
            .ok_or(HostError::Message("Unknown timer ID"))
    }
}
