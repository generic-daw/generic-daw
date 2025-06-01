use generic_daw_core::{Clip, Position, audio_graph::NodeId};

#[derive(Debug)]
pub struct Track {
    pub id: NodeId,
    pub clips: Vec<Clip>,
}

impl Track {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            clips: Vec::new(),
        }
    }

    pub fn len(&self) -> Position {
        self.clips
            .iter()
            .map(|clip| clip.position().get_global_end())
            .max()
            .unwrap_or_default()
    }
}
