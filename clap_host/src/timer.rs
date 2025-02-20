use clack_extensions::timer::{PluginTimer, TimerId};
use clack_host::prelude::*;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Timers {
    durations: HashMap<TimerId, (Duration, Instant)>,
    next_id: u32,
}

impl Timers {
    pub fn tick_timers(
        &mut self,
        timer_ext: &PluginTimer,
        plugin: &mut PluginMainThreadHandle<'_>,
    ) -> Instant {
        let now = Instant::now();
        let mut next = now + Duration::from_millis(30);

        for (&id, (interval, tick)) in &mut self.durations {
            if *tick <= now {
                timer_ext.on_timer(plugin, id);
                *tick += *interval;
            } else if *tick < next {
                next = *tick;
            }
        }

        next
    }

    pub fn register(&mut self, interval: Duration) -> TimerId {
        let id = TimerId(self.next_id);
        self.next_id += 1;

        self.durations.insert(id, (interval, Instant::now()));

        id
    }

    pub fn unregister(&mut self, id: TimerId) -> Result<(), HostError> {
        self.durations
            .remove(&id)
            .map(|_| ())
            .ok_or(HostError::Message("Unknown timer ID"))
    }
}
