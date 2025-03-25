use crate::AudioGraphNode;
use generic_daw_utils::HoleyVec;

/// an entry in an `AudioGraph`
#[derive(Debug)]
pub struct AudioGraphEntry {
    /// the `AudioGraphNode` of this entry
    pub node: AudioGraphNode,
    /// what other nodes this entry's node depends on
    pub connections: HoleyVec<Vec<f32>>,
    /// this entry's node's cached output audio
    pub cache: Vec<f32>,
    /// this node's critical delay
    pub delay: usize,
}
