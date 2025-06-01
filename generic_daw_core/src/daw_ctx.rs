use crate::{AudioGraphNode, Clip, Master, Meter, MixerNode};
use async_channel::{Receiver, Sender};
use audio_graph::{AudioGraph, NodeId, NodeImpl as _};
use clap_host::AudioProcessor;
use generic_daw_utils::unique_id;
use log::trace;

unique_id!(version);

pub use version::Id as Version;

#[derive(Debug)]
pub enum Message {
    Action(NodeId, Action),

    Insert(AudioGraphNode),
    Remove(NodeId),
    Connect(NodeId, NodeId, oneshot::Sender<(NodeId, NodeId)>),
    Disconnect(NodeId, NodeId),

    Bpm(u16),
    Numerator(u8),
    TogglePlayback,
    ToggleMetronome,
    Reset,
    Sample(Version, usize),

    RequestAudioGraph(oneshot::Sender<AudioGraph<AudioGraphNode>>),
    AudioGraph(AudioGraph<AudioGraphNode>),
}

#[derive(Debug)]
pub enum Action {
    AddClip(Clip),
    RemoveClip(usize),

    NodeToggleEnabled,
    NodeVolumeChanged(f32),
    NodePanChanged(f32),
    PluginLoad(Box<AudioProcessor>),
    PluginRemove(usize),
    PluginMoved(usize, usize),
    PluginToggleEnabled(usize),
    PluginMixChanged(usize, f32),
}

#[derive(Clone, Copy, Debug)]
pub enum Update {
    LR(NodeId, [f32; 2]),
    Sample(Version, usize),
}

#[derive(Debug)]
pub struct State {
    pub meter: Meter,
    pub sender: Sender<Update>,
    pub receiver: Receiver<Message>,
}

pub struct DawCtx {
    audio_graph: AudioGraph<AudioGraphNode>,
    state: State,
}

impl DawCtx {
    pub fn create(
        sample_rate: u32,
        buffer_size: u32,
    ) -> (Self, NodeId, Meter, Sender<Message>, Receiver<Update>) {
        let (message_sender, message_receiver) = async_channel::unbounded();
        let (update_sender, update_receiver) = async_channel::unbounded();

        let meter = Meter::new(sample_rate, buffer_size);
        let state = State {
            meter,
            sender: update_sender,
            receiver: message_receiver,
        };

        let node = MixerNode::default();
        let id = node.id();
        let audio_graph = AudioGraph::new(Master::new(meter.sample_rate, node).into());

        let audio_ctx = Self { audio_graph, state };

        (audio_ctx, id, meter, message_sender, update_receiver)
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        while let Ok(msg) = self.state.receiver.try_recv() {
            trace!("{msg:?}");

            match msg {
                Message::Action(node, action) => self.audio_graph.apply(node, action),
                Message::Insert(node) => self.audio_graph.insert(node),
                Message::Remove(node) => self.audio_graph.remove(node),
                Message::Connect(from, to, sender) => {
                    if self.audio_graph.connect(from, to) {
                        sender.send((from, to)).unwrap();
                    }
                }
                Message::Disconnect(from, to) => self.audio_graph.disconnect(from, to),
                Message::Bpm(bpm) => self.state.meter.bpm = bpm,
                Message::Numerator(numerator) => self.state.meter.numerator = numerator,
                Message::TogglePlayback => self.state.meter.playing ^= true,
                Message::ToggleMetronome => self.state.meter.metronome ^= true,
                Message::Reset => self.audio_graph.reset(),
                Message::Sample(_, sample) => self.state.meter.sample = sample,
                Message::RequestAudioGraph(sender) => {
                    debug_assert!(self.state.receiver.is_empty());
                    let mut audio_graph = AudioGraph::new(MixerNode::default().into());
                    std::mem::swap(&mut self.audio_graph, &mut audio_graph);
                    sender.send(audio_graph).unwrap();
                }
                Message::AudioGraph(audio_graph) => self.audio_graph = audio_graph,
            }
        }

        self.audio_graph.process(&self.state, buf);

        for s in &mut *buf {
            *s = s.clamp(-1.0, 1.0);
        }

        if self.state.meter.playing {
            self.state.meter.sample += buf.len();
            self.state
                .sender
                .try_send(Update::Sample(Version::last(), self.state.meter.sample))
                .unwrap();
        }
    }
}
