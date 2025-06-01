use crate::NodeImpl;
use generic_daw_utils::HoleyVec;

/// an entry in an `AudioGraph`
#[derive(Debug)]
pub struct Entry<Node: NodeImpl> {
    /// the node of this entry
    pub node: Node,
    /// what other nodes this entry's node depends on
    pub connections: HoleyVec<(Vec<f32>, Vec<Node::Event>)>,
    /// this entry's node's cached audio
    pub audio: Vec<f32>,
    /// this entry's node's cached events
    pub events: Vec<Node::Event>,
    /// this node's delay
    pub delay: usize,
}

impl<Node: NodeImpl> Entry<Node> {
    pub fn new(node: Node) -> Self {
        Self {
            node,
            connections: HoleyVec::default(),
            audio: Vec::new(),
            events: Vec::new(),
            delay: 0,
        }
    }
}
