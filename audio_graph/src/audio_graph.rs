use crate::{
	EventImpl as _, NodeId, NodeImpl,
	entry::{Entry, Incoming},
};
use crossbeam_queue::ArrayQueue;
use std::{
	collections::HashMap,
	num::NonZero,
	sync::{RwLockWriteGuard, atomic::Ordering::Relaxed},
};
use thread_pool::{ThreadPool, WorkList};
use utils::{NoDebug, boxed_slice};

impl<Node: NodeImpl> WorkList for AudioGraph<Node> {
	type Item = NodeId;
	type Scratch = Box<[f32]>;

	fn next_item(&self) -> Option<Self::Item> {
		self.queue.pop()
	}

	fn do_work(&self, item: Self::Item, scratch: &mut Self::Scratch) -> Option<Self::Item> {
		self.process_node(item, scratch)
	}
}

#[derive(Debug)]
pub struct AudioGraph<Node: NodeImpl> {
	state: Node::State,
	graph: HashMap<NodeId, Entry<Node>>,
	pool: Option<NoDebug<ThreadPool<Self>>>,
	queue: ArrayQueue<NodeId>,
	root: NodeId,
	frames: NonZero<u32>,
	len: usize,
}

impl<Node: NodeImpl> AudioGraph<Node> {
	#[must_use]
	pub fn new(state: Node::State, root: impl Into<Node>, frames: NonZero<u32>) -> Self {
		let root = root.into();

		let mut this = Self {
			state,
			graph: HashMap::new(),
			pool: Some(
				ThreadPool::new_with_scratch(|| boxed_slice![0.0; 2 * frames.get() as usize])
					.into(),
			),
			queue: ArrayQueue::new(4),
			root: root.id(),
			frames,
			len: 0,
		};

		this.insert(root);

		this
	}

	pub fn process(&mut self, buf: &mut [f32]) {
		self.len = buf.len();

		for (id, entry) in &mut self.graph {
			let indegree = entry.buffers().incoming.len().cast_signed();
			*entry.indegree.get_mut() = indegree - 1;
			if indegree == 0 {
				self.queue.push(*id).unwrap();
			}
		}

		let mut pool = self.pool.take().unwrap();
		pool.run(self, self.graph.len());
		self.pool = Some(pool);

		if cfg!(debug_assertions) {
			for entry in self.graph.values_mut() {
				assert_eq!(*entry.indegree.get_mut(), -1);
			}
		}

		buf.copy_from_slice(&self.entry_mut(self.root()).buffers().audio[..buf.len()]);
	}

	#[expect(clippy::significant_drop_tightening)]
	pub(crate) fn process_node(&self, node: NodeId, scratch: &mut [f32]) -> Option<NodeId> {
		let entry = &self.graph[&node];

		debug_assert_eq!(entry.indegree.load(Relaxed), -1);

		let mut buffers_lock = entry.write_buffers_uncontended();
		let buffers = &mut *buffers_lock;

		buffers.audio[..self.len].fill(0.0);
		buffers.events.clear();

		let max_delay = buffers
			.incoming
			.keys()
			.map(|node| self.graph[node].delay.load(Relaxed))
			.max()
			.unwrap_or_default();

		for (dep, Incoming { delay, events, mix }) in &mut buffers.incoming {
			let dep_entry = &self.graph[dep];
			let dep_buffers = &*dep_entry.read_buffers_uncontended();
			let delay_diff = max_delay - dep_entry.delay.load(Relaxed);
			delay.resize(delay_diff);

			let audio = if delay_diff == 0 {
				&dep_buffers.audio[..self.len]
			} else {
				scratch[..self.len].copy_from_slice(&dep_buffers.audio[..self.len]);
				delay.advance(&mut scratch[..self.len]);
				&scratch[..self.len]
			};

			audio
				.iter()
				.zip(&mut buffers.audio[..self.len])
				.for_each(|(&sample, buf)| *buf += *mix * sample);

			events.extend(
				dep_buffers
					.events
					.iter()
					.map(|e| e.at(e.time() + delay_diff)),
			);

			buffers.events.extend(events.extract_if(.., |e| {
				e.time()
					.checked_sub(self.len)
					.map(|time| *e = e.at(time))
					.is_none()
			}));
		}

		let mut node = entry.node_uncontended();

		node.process(
			&self.state,
			&mut buffers.audio[..self.len],
			&mut buffers.events,
		);
		entry.delay.store(node.delay() + max_delay, Relaxed);

		drop(node);

		let buffers_lock = RwLockWriteGuard::downgrade(buffers_lock);

		let mut iter = buffers_lock
			.outgoing
			.iter()
			.copied()
			.filter(|node| self.graph[node].indegree.fetch_sub(1, Relaxed) == 0);

		let inline = iter.next();

		for node in iter {
			self.queue.push(node).unwrap();
		}

		inline
	}

	pub fn state(&self) -> &Node::State {
		&self.state
	}

	pub fn state_mut(&mut self) -> &mut Node::State {
		&mut self.state
	}

	#[must_use]
	pub fn root(&self) -> NodeId {
		self.root
	}

	fn entry_mut(&mut self, node: NodeId) -> &mut Entry<Node> {
		self.graph.get_mut(&node).unwrap()
	}

	pub fn for_node_mut(&mut self, node: NodeId, f: impl FnOnce(&mut Node)) {
		f(&mut *self.entry_mut(node).node());
	}

	pub fn for_each_node_mut(&mut self, mut f: impl FnMut(&mut Node)) {
		for entry in self.graph.values_mut() {
			f(&mut *entry.node());
		}
	}

	#[must_use]
	pub fn delay(&self) -> usize {
		self.graph[&self.root()].delay.load(Relaxed)
	}

	#[must_use]
	pub fn connect(&mut self, from: NodeId, to: NodeId) -> bool {
		if !self.graph.contains_key(&from) || !self.graph.contains_key(&to) {
			return false;
		}

		if !self.entry_mut(from).buffers().outgoing.insert(to) {
			return true;
		}

		self.entry_mut(to)
			.buffers()
			.incoming
			.insert(from, Incoming::default());

		if self.has_cycle() {
			self.entry_mut(from).buffers().outgoing.remove(&to);
			self.entry_mut(to).buffers().incoming.remove(&from);
			return false;
		}

		true
	}

	pub fn set_mix(&mut self, from: NodeId, to: NodeId, mix: f32) {
		self.entry_mut(to)
			.buffers()
			.incoming
			.get_mut(&from)
			.unwrap()
			.mix = mix;
	}

	pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
		self.entry_mut(from).buffers().outgoing.remove(&to);
		self.entry_mut(to).buffers().incoming.remove(&from);
	}

	pub fn insert(&mut self, node: Node) {
		let id = node.id();

		if let Some(entry) = self.graph.get_mut(&id) {
			*entry.node() = node;
		} else {
			self.graph.insert(id, Entry::new(node, self.frames));
			if self.queue.capacity() < self.graph.len() {
				self.queue = ArrayQueue::new(2 * self.queue.capacity());
			}
		}
	}

	pub fn remove(&mut self, node: NodeId) {
		if let Some(mut entry) = self.graph.remove(&node) {
			for &incoming in entry.buffers().incoming.keys() {
				self.entry_mut(incoming).buffers().outgoing.remove(&node);
			}

			for &outgoing in &entry.buffers().outgoing {
				self.entry_mut(outgoing).buffers().incoming.remove(&node);
			}
		}
	}

	fn has_cycle(&mut self) -> bool {
		for entry in self.graph.values_mut() {
			*entry.indegree.get_mut() = entry.buffers().incoming.len().cast_signed();
		}

		self.graph
			.values()
			.filter(|entry| entry.indegree.fetch_sub(1, Relaxed) == 0)
			.for_each(|entry| self.visit(entry));

		!self
			.graph
			.values_mut()
			.all(|entry| *entry.indegree.get_mut() == -1)
	}

	fn visit(&self, entry: &Entry<Node>) {
		debug_assert_eq!(entry.indegree.load(Relaxed), -1);

		entry
			.read_buffers_uncontended()
			.outgoing
			.iter()
			.map(|node| &self.graph[node])
			.filter(|dep| dep.indegree.fetch_sub(1, Relaxed) == 0)
			.for_each(|dep| self.visit(dep));
	}
}
