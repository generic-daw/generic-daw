use crate::{NodeId, NodeImpl};
use dsp::DelayLine;
use std::{
	collections::{HashMap, HashSet},
	num::NonZero,
	sync::{
		Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
		atomic::{AtomicIsize, AtomicUsize},
	},
};
use utils::{NoDebug, boxed_slice};

#[derive(Debug)]
pub struct Entry<Node: NodeImpl> {
	node: Mutex<Node>,
	buffers: RwLock<Buffers<Node>>,
	pub indegree: AtomicIsize,
	pub delay: AtomicUsize,
}

#[derive(Debug)]
pub struct Buffers<Node: NodeImpl> {
	pub incoming: HashMap<NodeId, (DelayLine, Vec<Node::Event>)>,
	pub outgoing: HashSet<NodeId>,
	pub audio: NoDebug<Box<[f32]>>,
	pub events: Vec<Node::Event>,
}

impl<Node: NodeImpl> Entry<Node> {
	pub fn new(node: Node, frames: NonZero<u32>) -> Self {
		Self {
			node: Mutex::new(node),
			buffers: RwLock::new(Buffers {
				incoming: HashMap::new(),
				outgoing: HashSet::new(),
				audio: boxed_slice![0.0; 2 * frames.get() as usize].into(),
				events: Vec::new(),
			}),
			indegree: AtomicIsize::new(0),
			delay: AtomicUsize::new(0),
		}
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
