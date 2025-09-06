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
	pub peaks: [[peak_meter::State; 2]; 2],
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
			peaks: [
				[peak_meter::State::new(1.0), peak_meter::State::new(1.0)],
				[peak_meter::State::new(3.0), peak_meter::State::new(3.0)],
			],
			enabled: true,
			volume: 1.0,
			pan: 0.0,
			plugins: Vec::new(),
		}
	}

	pub fn update(&mut self, peaks: [f32; 2], now: Instant) {
		self.peaks[0][0].update(peaks[0], now);
		self.peaks[0][1].update(peaks[1], now);
		self.peaks[1][0].update(peaks[0].cbrt(), now);
		self.peaks[1][1].update(peaks[1].cbrt(), now);
	}
}
