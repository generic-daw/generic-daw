use crate::NodeImpl;
use generic_daw_utils::{AudioRingbuf, HoleyVec};

#[derive(Debug)]
pub struct Entry<Node: NodeImpl> {
	pub node: Node,
	pub connections: HoleyVec<(AudioRingbuf, Vec<Node::Event>)>,
	pub audio: Box<[f32]>,
	pub events: Vec<Node::Event>,
	pub delay: usize,
}

impl<Node: NodeImpl> Entry<Node> {
	pub fn new(node: Node, frames: u32) -> Self {
		Self {
			node,
			connections: HoleyVec::default(),
			audio: vec![0.0; 2 * frames as usize].into_boxed_slice(),
			events: Vec::new(),
			delay: 0,
		}
	}
}
