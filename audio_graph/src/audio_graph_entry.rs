use generic_daw_utils::HoleyVec;

/// an entry in an `AudioGraph`
#[derive(Debug)]
pub struct AudioGraphEntry<N, S, E> {
    /// the node of this entry
    pub node: N,
    /// what other nodes this entry's node depends on
    pub connections: HoleyVec<Vec<S>>,
    /// this entry's node's cached audio
    pub audio: Vec<S>,
    /// this entry's node's cached events
    pub events: Vec<E>,
    /// this node's delay
    pub delay: usize,
}

impl<N, S, E> AudioGraphEntry<N, S, E> {
    pub fn new(node: N) -> Self {
        Self {
            node,
            connections: HoleyVec::default(),
            audio: Vec::new(),
            events: Vec::new(),
            delay: 0,
        }
    }
}
