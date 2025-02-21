use clack_extensions::timer::{PluginTimer, TimerId};
use clack_host::prelude::*;
use generic_daw_utils::HoleyVec;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Timers {
    durations: HoleyVec<(Duration, Instant)>,
    next_id: usize,
}

impl Timers {
    pub fn tick_timers(
        &mut self,
        timer_ext: &PluginTimer,
        plugin: &mut PluginMainThreadHandle<'_>,
    ) -> Instant {
        let now = Instant::now();
        let mut next = now + Duration::from_millis(30);

        for (id, (interval, tick)) in self.durations.iter_mut() {
            if *tick <= now {
                timer_ext.on_timer(plugin, TimerId(id as u32));
                *tick += *interval;
            } else if *tick < next {
                next = *tick;
            }
        }

        next
    }

    pub fn register(&mut self, interval: Duration) -> TimerId {
        let id = TimerId(self.next_id as u32);

        self.durations
            .insert(self.next_id, (interval, Instant::now()));
        self.next_id += 1;

        id
    }

    pub fn unregister(&mut self, id: TimerId) -> Result<(), HostError> {
        self.durations
            .remove(id.0 as usize)
            .map(|_| ())
            .ok_or(HostError::Message("Unknown timer ID"))
    }
}
