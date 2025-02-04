use audio_graph::{AudioGraphNode, NodeId};

#[derive(Debug)]
pub enum AudioCtxMessage {
    Add(AudioGraphNode),
    Remove(NodeId),
    Connect(NodeId, NodeId),
    Disconnect(NodeId, NodeId),
}
