use crate::{Action, Update, daw_ctx::State, event::Event};
use audio_graph::{NodeId, NodeImpl};
use clap_host::AudioProcessor;
use generic_daw_utils::ShiftMoveExt as _;
use std::f32::consts::{FRAC_PI_4, SQRT_2};

#[derive(Debug)]
struct Plugin {
    processor: AudioProcessor,
    mix: f32,
    enabled: bool,
}

impl Plugin {
    pub fn new(processor: AudioProcessor) -> Self {
        Self {
            processor,
            mix: 1.0,
            enabled: true,
        }
    }
}

#[derive(Debug)]
pub struct Mixer {
    id: NodeId,
    plugins: Vec<Plugin>,
    volume: f32,
    pan: f32,
    enabled: bool,
}

impl NodeImpl for Mixer {
    type Event = Event;
    type State = State;

    fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
        if !self.enabled {
            for s in audio {
                *s = 0.0;
            }
            events.clear();

            return;
        }

        self.plugins
            .iter_mut()
            .filter(|entry| entry.enabled)
            .for_each(|entry| {
                entry.processor.process(audio, events, entry.mix);
            });

        let [lpan, rpan] = pan(self.pan).map(|s| s * self.volume);

        let l_r = (audio.as_chunks_mut().0).iter_mut().fold(
            [0.0; 2],
            |[old_l, old_r], [new_l, new_r]| {
                *new_l *= lpan;
                *new_r *= rpan;
                [new_l.abs().max(old_l), new_r.abs().max(old_r)]
            },
        );

        if l_r != [0.0; 2] {
            _ = state.sender.try_send(Update::LR(self.id, l_r));
        }
    }

    fn id(&self) -> NodeId {
        self.id
    }

    fn reset(&mut self) {
        for plugin in &mut self.plugins {
            plugin.processor.reset();
        }
    }

    fn delay(&self) -> usize {
        self.plugins
            .iter()
            .map(|entry| entry.processor.delay())
            .sum()
    }
}

impl Mixer {
    pub fn apply(&mut self, action: Action) {
        match action {
            Action::NodeToggleEnabled => self.enabled ^= true,
            Action::NodeVolumeChanged(volume) => self.volume = volume,
            Action::NodePanChanged(pan) => self.pan = pan,
            Action::PluginLoad(processor) => {
                self.plugins.push(Plugin::new(*processor));
            }
            Action::PluginRemove(index) => {
                self.plugins.remove(index);
            }
            Action::PluginMoved(from, to) => self.plugins.shift_move(from, to),
            Action::PluginToggleEnabled(index) => self.plugins[index].enabled ^= true,
            Action::PluginMixChanged(index, mix) => self.plugins[index].mix = mix,
            _ => panic!(),
        }
    }
}

impl Default for Mixer {
    fn default() -> Self {
        Self {
            plugins: Vec::new(),
            id: NodeId::unique(),
            volume: 1.0,
            pan: 0.0,
            enabled: true,
        }
    }
}

fn pan(pan: f32) -> [f32; 2] {
    let angle = (pan + 1.0) * FRAC_PI_4;

    [angle.cos() * SQRT_2, angle.sin() * SQRT_2]
}
