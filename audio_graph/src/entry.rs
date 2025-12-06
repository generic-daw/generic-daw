use crate::NodeImpl;
use bit_set::BitSet;
use dsp::DelayLine;
use std::{
	num::NonZero,
	sync::{
		Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
		atomic::{AtomicIsize, AtomicUsize},
	},
};
use utils::{HoleyVec, NoDebug};

#[derive(Debug)]
pub struct Entry<Node: NodeImpl> {
	node: Mutex<Node>,
	buffers: RwLock<Buffers<Node>>,
	pub indegree: AtomicIsize,
	pub delay: AtomicUsize,
	pub expensive: bool,
}

#[derive(Debug)]
pub struct Buffers<Node: NodeImpl> {
	pub incoming: HoleyVec<(DelayLine, Vec<Node::Event>)>,
	pub outgoing: BitSet,
	pub audio: NoDebug<Box<[f32]>>,
	pub buf: NoDebug<Box<[f32]>>,
	pub events: Vec<Node::Event>,
}

impl<Node: NodeImpl> Entry<Node> {
	pub fn new(node: Node, frames: NonZero<u32>) -> Self {
		Self {
			node: Mutex::new(node),
			buffers: RwLock::new(Buffers {
				incoming: HoleyVec::default(),
				outgoing: BitSet::default(),
				audio: vec![0.0; 2 * frames.get() as usize]
					.into_boxed_slice()
					.into(),
				buf: vec![0.0; 2 * frames.get() as usize]
					.into_boxed_slice()
					.into(),
				events: Vec::new(),
			}),
			indegree: AtomicIsize::new(0),
			delay: AtomicUsize::new(0),
			expensive: true,
		}
	}

	pub fn node(&mut self) -> &mut Node {
		self.node.get_mut().unwrap()
	}

	pub fn node_uncontended(&self) -> MutexGuard<'_, Node> {
		self.node.lock().expect("this is always uncontended")
	}

	pub fn buffers(&mut self) -> &mut Buffers<Node> {
		self.buffers.get_mut().unwrap()
	}

	pub fn read_buffers_uncontended(&self) -> RwLockReadGuard<'_, Buffers<Node>> {
		self.buffers.read().expect("this is always uncontended")
	}

	pub fn write_buffers_uncontended(&self) -> RwLockWriteGuard<'_, Buffers<Node>> {
		self.buffers.write().expect("this is always uncontended")
	}
}
