use clack_extensions::timer::{PluginTimer, TimerId};
use clack_host::prelude::*;
use std::{
    collections::{BTreeSet, HashMap},
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Timers {
    durations: HashMap<TimerId, Duration>,
    ticks: BTreeSet<(Instant, TimerId)>,
    next_id: u32,
}

impl Timers {
    pub fn tick_timers(
        &mut self,
        timer_ext: &PluginTimer,
        plugin: &mut PluginMainThreadHandle<'_>,
    ) -> Option<Instant> {
        let now = Instant::now();

        while self.ticks.first().is_some_and(|t| t.0 < now) {
            let (_, id) = self.ticks.pop_first().unwrap();

            timer_ext.on_timer(plugin, id);

            let next_tick = now + self.durations[&id];
            self.ticks.insert((next_tick, id));
        }

        self.ticks.first().map(|t| t.0)
    }

    pub fn register(&mut self, interval: Duration) -> TimerId {
        let now = Instant::now();
        let id = TimerId(self.next_id);
        self.next_id += 1;

        self.durations.insert(id, interval);
        self.ticks.insert((now + interval, id));

        id
    }

    pub fn unregister(&mut self, id: TimerId) -> Result<(), HostError> {
        self.durations
            .remove(&id)
            .map(|_| {
                self.ticks.retain(|&(_, tid)| tid != id);
            })
            .ok_or(HostError::Message("Unknown timer ID"))
    }
}
