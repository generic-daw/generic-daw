use crate::{AudioGraphNode, Clip, Master, Mixer};
use async_channel::{Receiver, Sender};
use audio_graph::{AudioGraph, NodeId};
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

#[derive(Clone, Copy, Debug)]
pub struct RtState {
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub bpm: u16,
    pub numerator: u8,
    pub playing: bool,
    pub metronome: bool,
    pub sample: usize,
}

#[derive(Debug)]
pub struct State {
    pub rtstate: RtState,
    pub sender: Sender<Update>,
    pub receiver: Receiver<Message>,
}

pub struct DawCtx {
    audio_graph: AudioGraph<AudioGraphNode>,
    state: State,
    version: Version,
}

impl DawCtx {
    pub fn create(
        sample_rate: u32,
        buffer_size: u32,
    ) -> (Self, NodeId, RtState, Sender<Message>, Receiver<Update>) {
        let (r_sender, receiver) = async_channel::unbounded();
        let (sender, r_receiver) = async_channel::unbounded();

        let state = State {
            rtstate: RtState {
                sample_rate,
                buffer_size,
                bpm: 140,
                numerator: 4,
                playing: false,
                metronome: false,
                sample: 0,
            },
            sender,
            receiver,
        };

        let audio_graph = AudioGraph::new(Master::new(state.rtstate.sample_rate).into());
        let id = audio_graph.root();

        let audio_ctx = Self {
            audio_graph,
            state,
            version: Version::unique(),
        };
        let rtstate = audio_ctx.state.rtstate;

        (audio_ctx, id, rtstate, r_sender, r_receiver)
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        while let Ok(msg) = self.state.receiver.try_recv() {
            trace!("{msg:?}");

            match msg {
                Message::Action(node, action) => {
                    if let Some(node) = self.audio_graph.node_mut(node) {
                        node.apply(action);
                    }
                }
                Message::Insert(node) => self.audio_graph.insert(node),
                Message::Remove(node) => self.audio_graph.remove(node),
                Message::Connect(from, to, sender) => {
                    if self.audio_graph.connect(from, to) {
                        sender.send((from, to)).unwrap();
                    }
                }
                Message::Disconnect(from, to) => self.audio_graph.disconnect(from, to),
                Message::Bpm(bpm) => self.state.rtstate.bpm = bpm,
                Message::Numerator(numerator) => self.state.rtstate.numerator = numerator,
                Message::TogglePlayback => self.state.rtstate.playing ^= true,
                Message::ToggleMetronome => self.state.rtstate.metronome ^= true,
                Message::Reset => self.audio_graph.reset(),
                Message::Sample(version, sample) => {
                    self.state.rtstate.sample = sample;
                    self.version = version;
                }
                Message::RequestAudioGraph(sender) => {
                    debug_assert!(self.state.receiver.is_empty());
                    let mut audio_graph = AudioGraph::new(Mixer::default().into());
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

        if self.state.rtstate.playing {
            self.state.rtstate.sample += buf.len();
            self.state
                .sender
                .try_send(Update::Sample(self.version, self.state.rtstate.sample))
                .unwrap();
        }
    }
}
