use crate::NodeImpl;
use generic_daw_utils::HoleyVec;

#[derive(Debug)]
pub struct Entry<Node: NodeImpl> {
	pub node: Node,
	pub connections: HoleyVec<(Vec<f32>, Vec<Node::Event>)>,
	pub audio: Vec<f32>,
	pub events: Vec<Node::Event>,
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
