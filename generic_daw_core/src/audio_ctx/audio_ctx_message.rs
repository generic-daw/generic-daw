use audio_graph::AudioGraphNode;

#[derive(Debug)]
pub enum AudioCtxMessage {
    Add(AudioGraphNode),
    Remove(AudioGraphNode),
    Connect(AudioGraphNode, AudioGraphNode),
    Disconnect(AudioGraphNode, AudioGraphNode),
}
