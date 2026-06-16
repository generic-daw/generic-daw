use crate::{NodeId, NodeImpl};
use dsp::DelayLine;
use std::{
	collections::{HashMap, HashSet},
	num::NonZero,
	sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, atomic::AtomicUsize},
};
use utils::{NoDebug, boxed_slice};

#[derive(Debug)]
pub struct Incoming<Node: NodeImpl> {
	pub delay_line: DelayLine,
	pub events: Vec<Node::Event>,
	pub mix: f32,
}

impl<Node: NodeImpl> Default for Incoming<Node> {
	fn default() -> Self {
		Self {
			delay_line: DelayLine::default(),
			events: Vec::default(),
			mix: 1.0,
		}
	}
}

#[derive(Debug)]
pub struct Entry<Node: NodeImpl> {
	node: Mutex<Node>,
	buffers: RwLock<Buffers<Node>>,
	pub indegree: AtomicUsize,
	pub latency: AtomicUsize,
}

#[derive(Debug)]
pub struct Buffers<Node: NodeImpl> {
	pub incoming: HashMap<NodeId, Incoming<Node>>,
	pub outgoing: HashSet<NodeId>,
	pub audio: NoDebug<Box<[[f32; 2]]>>,
	pub events: Vec<Node::Event>,
}

impl<Node: NodeImpl> Entry<Node> {
	pub fn new(node: Node, frames: NonZero<u32>) -> Self {
		Self {
			node: Mutex::new(node),
			buffers: RwLock::new(Buffers {
				incoming: HashMap::new(),
				outgoing: HashSet::new(),
				audio: boxed_slice![[0.0; 2]; frames.get() as usize].into(),
				events: Vec::new(),
			}),
			indegree: AtomicUsize::new(0),
			latency: AtomicUsize::new(0),
		}
	}

	pub fn change_max_frames(&mut self, frames: NonZero<u32>) {
		self.buffers().audio = boxed_slice![[0.0; 2]; frames.get() as usize].into();
	}

	pub fn node(&mut self) -> &mut Node {
		self.node.get_mut().unwrap()
	}

	pub fn node_uncontended(&self) -> MutexGuard<'_, Node> {
		self.node.try_lock().expect("this is always uncontended")
	}

	pub fn buffers(&mut self) -> &mut Buffers<Node> {
		self.buffers.get_mut().unwrap()
	}

	pub fn read_buffers_uncontended(&self) -> RwLockReadGuard<'_, Buffers<Node>> {
		self.buffers.try_read().expect("this is always uncontended")
	}

	pub fn write_buffers_uncontended(&self) -> RwLockWriteGuard<'_, Buffers<Node>> {
		self.buffers
			.try_write()
			.expect("this is always uncontended")
	}
}
