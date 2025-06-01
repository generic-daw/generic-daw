use super::plugin::Plugin;
use generic_daw_core::audio_graph::NodeId;
use std::cell::Cell;

#[derive(Debug)]
pub struct Node {
    pub id: NodeId,
    pub l_r: Cell<[f32; 2]>,
    pub enabled: bool,
    pub volume: f32,
    pub pan: f32,
    pub plugins: Vec<Plugin>,
}

impl Node {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            l_r: Cell::default(),
            enabled: true,
            volume: 1.0,
            pan: 0.0,
            plugins: Vec::new(),
        }
    }
}
