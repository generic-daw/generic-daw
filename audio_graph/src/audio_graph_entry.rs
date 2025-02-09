use crate::AudioGraphNode;
use bit_set::BitSet;

#[derive(Debug)]
pub struct AudioGraphEntry {
    pub node: AudioGraphNode,
    pub connections: BitSet,
    pub cache: Vec<f32>,
}
