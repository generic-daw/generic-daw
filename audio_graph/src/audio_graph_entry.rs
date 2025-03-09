use crate::AudioGraphNode;
use bit_set::BitSet;

/// an entry in an `AudioGraph`
#[derive(Debug)]
pub struct AudioGraphEntry {
    /// the `AudioGraphNode` of this entry
    pub node: AudioGraphNode,
    /// what other nodes this entry's node depends on
    pub connections: BitSet,
    /// this entry's node's cached output audio
    pub cache: Vec<f32>,
}
