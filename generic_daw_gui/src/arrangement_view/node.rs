use super::plugin::Plugin;
use generic_daw_core::NodeId;
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeType {
	Master,
	Mixer,
	Track,
}

#[derive(Debug)]
pub struct Node {
	pub ty: NodeType,
	pub id: NodeId,
	pub l_r: Cell<[f32; 2]>,
	pub enabled: bool,
	pub volume: f32,
	pub pan: f32,
	pub plugins: Vec<Plugin>,
}

impl Node {
	pub fn new(ty: NodeType, id: NodeId) -> Self {
		Self {
			ty,
			id,
			l_r: Cell::default(),
			enabled: true,
			volume: 1.0,
			pan: 0.0,
			plugins: Vec::new(),
		}
	}
}
