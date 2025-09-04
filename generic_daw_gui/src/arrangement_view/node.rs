use super::plugin::Plugin;
use generic_daw_core::NodeId;
use generic_daw_widget::peak_meter;
use std::time::Instant;

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
	pub peak: [[peak_meter::State; 2]; 2],
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
			peak: [
				[peak_meter::State::new(1.0), peak_meter::State::new(1.0)],
				[peak_meter::State::new(3.0), peak_meter::State::new(3.0)],
			],
			enabled: true,
			volume: 1.0,
			pan: 0.0,
			plugins: Vec::new(),
		}
	}

	pub fn update(&mut self, l_r: [f32; 2], now: Instant) {
		self.peak[0][0].update(l_r[0], now);
		self.peak[0][1].update(l_r[1], now);
		self.peak[1][0].update(l_r[0].cbrt(), now);
		self.peak[1][1].update(l_r[1].cbrt(), now);
	}
}
