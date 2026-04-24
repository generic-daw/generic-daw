use generic_daw_core::NodeId;

#[derive(Debug)]
pub struct Channel {
	pub id: NodeId,
}

impl Channel {
	pub fn new(id: NodeId) -> Self {
		Self { id }
	}
}
