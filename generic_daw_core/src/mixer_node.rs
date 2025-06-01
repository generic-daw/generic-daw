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
pub struct MixerNode {
    id: NodeId,
    plugins: Vec<Plugin>,
    volume: f32,
    pan: f32,
    enabled: bool,
}

impl NodeImpl for MixerNode {
    type Action = Action;
    type Event = Event;
    type State = State;

    fn apply(&mut self, action: Self::Action) {
        match action {
            Self::Action::NodeToggleEnabled => self.enabled ^= true,
            Self::Action::NodeVolumeChanged(volume) => self.volume = volume,
            Self::Action::NodePanChanged(pan) => self.pan = pan,
            Self::Action::PluginLoad(processor) => {
                self.plugins.push(Plugin::new(*processor));
            }
            Self::Action::PluginRemove(index) => {
                self.plugins.remove(index);
            }
            Self::Action::PluginMoved(from, to) => self.plugins.shift_move(from, to),
            Self::Action::PluginToggleEnabled(index) => self.plugins[index].enabled ^= true,
            Self::Action::PluginMixChanged(index, mix) => self.plugins[index].mix = mix,
            _ => panic!(),
        }
    }

    fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
        if !self.enabled {
            audio.iter_mut().for_each(|s| *s = 0.0);
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

        let l_r = audio.chunks_exact_mut(2).fold([0.0; 2], |[l, r], cur| {
            cur[0] *= lpan;
            cur[1] *= rpan;

            [cur[0].abs().max(l), cur[1].abs().max(r)]
        });

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

impl Default for MixerNode {
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

    let (r, l) = angle.sin_cos();

    [l * SQRT_2, r * SQRT_2]
}
