use audio_graph::{AudioGraph, AudioGraphNode, NodeId};
use oneshot::Sender;

#[derive(Debug)]
pub enum DawCtxMessage<T> {
    Insert(AudioGraphNode),
    Remove(NodeId),
    Connect(NodeId, NodeId),
    ConnectToMaster(NodeId),
    Disconnect(NodeId, NodeId),
    DisconnectFromMaster(NodeId),
    RequestAudioGraph(Sender<(AudioGraph, T)>, T),
    AudioGraph(AudioGraph),
}
