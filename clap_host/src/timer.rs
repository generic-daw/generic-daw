use clack_extensions::timer::{PluginTimer, TimerId};
use clack_host::prelude::*;
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeSet, HashMap},
    time::{Duration, Instant},
};

pub struct Timers {
    durations: RefCell<HashMap<TimerId, Duration>>,
    ticks: RefCell<BTreeSet<(Instant, TimerId)>>,
    next_id: Cell<u32>,
}

impl Default for Timers {
    fn default() -> Self {
        Self {
            durations: RefCell::default(),
            ticks: RefCell::default(),
            next_id: Cell::new(0),
        }
    }
}

impl Timers {
    pub fn tick_timers(
        &self,
        timer_ext: &PluginTimer,
        plugin: &mut PluginMainThreadHandle<'_>,
    ) -> Option<Instant> {
        let now = Instant::now();

        while self.ticks.borrow().first().is_some_and(|t| t.0 < now) {
            let (_, id) = self.ticks.borrow_mut().pop_first().unwrap();

            timer_ext.on_timer(plugin, id);

            let next_tick = now + self.durations.borrow()[&id];
            self.ticks.borrow_mut().insert((next_tick, id));
        }

        self.ticks.borrow().first().map(|t| t.0)
    }

    pub fn register(&self, interval: Duration) -> TimerId {
        let now = Instant::now();

        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let id = TimerId(id);

        self.durations.borrow_mut().insert(id, interval);
        self.ticks.borrow_mut().insert((now + interval, id));

        id
    }

    pub fn unregister(&self, id: TimerId) -> bool {
        self.durations
            .borrow_mut()
            .remove(&id)
            .inspect(|_| {
                self.ticks.borrow_mut().retain(|&(_, tid)| tid != id);
            })
            .is_some()
    }
}
