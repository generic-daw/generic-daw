use crate::{AudioGraphNode, NodeId};
use gxhash::HashSet;

#[derive(Debug)]
pub struct AudioGraphEntry {
    pub node: AudioGraphNode,
    pub connections: HashSet<NodeId>,
    pub cache: Vec<f32>,
}
