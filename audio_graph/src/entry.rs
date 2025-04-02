use generic_daw_utils::HoleyVec;

/// an entry in an `AudioGraph`
#[derive(Debug)]
pub struct Entry<Node, Event> {
    /// the node of this entry
    pub node: Node,
    /// what other nodes this entry's node depends on
    pub connections: HoleyVec<(Vec<f32>, Vec<Event>)>,
    /// this entry's node's cached audio
    pub audio: Vec<f32>,
    /// this entry's node's cached events
    pub events: Vec<Event>,
    /// this node's delay
    pub delay: usize,
}

impl<Node, Event> Entry<Node, Event> {
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
