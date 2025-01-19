use clack_extensions::timer::{PluginTimer, TimerId};
use clack_host::prelude::*;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    time::{Duration, Instant},
};

pub struct Timers {
    #[expect(clippy::struct_field_names)]
    timers: RefCell<HashMap<TimerId, Timer>>,
    latest_id: Cell<u32>,
    next_tick: Cell<Option<Instant>>,
}

impl Default for Timers {
    fn default() -> Self {
        Self {
            timers: RefCell::new(HashMap::new()),
            latest_id: Cell::new(0),
            next_tick: Cell::new(None),
        }
    }
}

impl Timers {
    fn tick_all(&self) -> Vec<TimerId> {
        let timers = self
            .timers
            .borrow_mut()
            .values_mut()
            .filter_map(|t| t.tick().then_some(t.id))
            .collect();

        self.next_tick
            .set(self.timers.borrow().values().map(Timer::next_tick).min());

        timers
    }

    pub fn tick_timers(&self, timer_ext: &PluginTimer, plugin: &mut PluginMainThreadHandle<'_>) {
        for triggered in self.tick_all() {
            timer_ext.on_timer(plugin, triggered);
        }
    }

    pub fn register_new(&self, interval: Duration) -> TimerId {
        let latest_id = self.latest_id.get() + 1;
        self.latest_id.set(latest_id);
        let id = TimerId(latest_id);

        self.timers
            .borrow_mut()
            .insert(id, Timer::new(id, interval));

        let next_tick = Instant::now() + interval;
        match self.next_tick.get() {
            None => self.next_tick.set(Some(next_tick)),
            Some(smallest) if smallest > next_tick => self.next_tick.set(Some(next_tick)),
            _ => {}
        }

        id
    }

    pub fn unregister(&self, id: TimerId) -> bool {
        let mut timers = self.timers.borrow_mut();
        if timers.remove(&id).is_some() {
            self.next_tick.set(
                timers
                    .values()
                    .map(|t| t.last_triggered_at.unwrap_or_else(Instant::now) + t.interval)
                    .min(),
            );
            true
        } else {
            false
        }
    }

    pub fn next_tick(&self) -> Option<Instant> {
        self.next_tick.get()
    }
}

struct Timer {
    id: TimerId,
    interval: Duration,
    last_triggered_at: Option<Instant>,
}

impl Timer {
    fn new(id: TimerId, interval: Duration) -> Self {
        Self {
            id,
            interval,
            last_triggered_at: None,
        }
    }

    fn tick(&mut self) -> bool {
        let now = Instant::now();
        let triggered = if let Some(last_updated_at) = self.last_triggered_at {
            if let Some(since) = now.checked_duration_since(last_updated_at) {
                since >= self.interval
            } else {
                false
            }
        } else {
            true
        };

        if triggered {
            self.last_triggered_at = Some(now);
        }

        triggered
    }

    fn next_tick(&self) -> Instant {
        self.last_triggered_at.unwrap_or_else(Instant::now) + self.interval
    }
}
