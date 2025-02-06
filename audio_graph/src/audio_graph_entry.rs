use crate::AudioGraphNode;
use bit_vec::BitVec;

#[derive(Debug)]
pub struct AudioGraphEntry {
    pub node: AudioGraphNode,
    pub connections: BitVec,
    pub cache: Vec<f32>,
}
