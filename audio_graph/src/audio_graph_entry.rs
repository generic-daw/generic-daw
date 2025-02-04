use crate::{AudioGraphNode, NodeId};
use ahash::AHashSet;

#[derive(Debug)]
pub struct AudioGraphEntry {
    pub node: AudioGraphNode,
    pub connections: AHashSet<NodeId>,
    pub cache: Vec<f32>,
}
