use audio_graph::{AudioGraph, AudioGraphNode, NodeId};

#[derive(Debug)]
pub enum AudioCtxMessage<T> {
    Insert(AudioGraphNode),
    Remove(NodeId),
    Connect(NodeId, NodeId),
    ConnectToMaster(NodeId),
    Disconnect(NodeId, NodeId),
    DisconnectFromMaster(NodeId),
    RequestAudioGraph(T),
    AudioGraph(AudioGraph),
}
