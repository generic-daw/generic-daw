use audio_graph::{AudioGraph, AudioGraphNode, NodeId};
use oneshot::Sender;

#[derive(Debug)]
pub enum DawCtxMessage {
    Insert(AudioGraphNode),
    Remove(NodeId),
    Connect(NodeId, NodeId, Sender<(NodeId, NodeId)>),
    Disconnect(NodeId, NodeId),
    RequestAudioGraph(Sender<AudioGraph>),
    Reset,
    AudioGraph(AudioGraph),
}
