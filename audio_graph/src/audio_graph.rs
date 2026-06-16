use crate::{
	EventImpl as _, Inject, NodeId, NodeImpl,
	entry::{Entry, Incoming},
};
use crossbeam_queue::ArrayQueue;
use std::{
	collections::HashMap,
	num::NonZero,
	sync::{RwLockWriteGuard, atomic::Ordering::Relaxed},
};
use thread_pool::{Injector, ThreadPool, WorkList};
use utils::{NoDebug, boxed_slice};

impl<Node: NodeImpl> WorkList for AudioGraph<Node> {
	type Item = NodeId;
	type Scratch = Box<[[f32; 2]]>;
	type Inject = Inject<Node>;

	fn next_item(&self) -> Option<Self::Item> {
		self.queue.pop()
	}

	fn do_work(
		&self,
		item: Self::Item,
		scratch: &mut Self::Scratch,
		injector: &Injector<Inject<Node>>,
	) -> Option<Self::Item> {
		self.process_node(&self.graph[&item], scratch, injector)
	}
}

#[derive(Debug)]
pub struct AudioGraph<Node: NodeImpl> {
	state: Node::State,
	graph: HashMap<NodeId, Entry<Node>>,
	pool: Option<NoDebug<ThreadPool<Self>>>,
	queue: ArrayQueue<NodeId>,
	max_frames: NonZero<u32>,
	curr_len: usize,
	needs_reset: bool,
}

impl<Node: NodeImpl> AudioGraph<Node> {
	#[must_use]
	pub fn new(state: Node::State, max_frames: NonZero<u32>) -> Self {
		Self {
			state,
			graph: HashMap::new(),
			pool: Some(
				ThreadPool::new_with_scratch(|| boxed_slice![[0.0; 2]; max_frames.get() as usize])
					.into(),
			),
			queue: ArrayQueue::new(4),
			max_frames,
			curr_len: 0,
			needs_reset: false,
		}
	}

	pub fn change_max_frames(&mut self, max_frames: NonZero<u32>) {
		if self.max_frames == max_frames {
			return;
		}
		self.graph
			.values_mut()
			.for_each(|entry| entry.change_max_frames(max_frames));
		self.pool = None;
		self.pool = Some(
			ThreadPool::new_with_scratch(|| boxed_slice![[0.0; 2]; max_frames.get() as usize])
				.into(),
		);
		self.max_frames = max_frames;
	}

	pub fn process_all(&mut self, node: NodeId, audio: &mut [[f32; 2]]) {
		self.curr_len = audio.len();

		for (&id, entry) in &mut self.graph {
			*entry.indegree.get_mut() = entry.buffers().incoming.len();
			if *entry.indegree.get_mut() == 0 {
				self.queue.push(id).unwrap();
			}
		}

		let mut pool = self.pool.take().unwrap();
		pool.run(
			self,
			self.graph.len(),
			NonZero::new(self.queue.len()).unwrap(),
		);
		self.pool = Some(pool);

		self.needs_reset = false;

		for entry in self.graph.values_mut() {
			debug_assert_eq!(*entry.indegree.get_mut(), 0);
		}

		self.copy_output(node, audio);
	}

	pub fn process_subtree(&mut self, node: NodeId, audio: &mut [[f32; 2]]) {
		fn visit<Node: NodeImpl>(this: &AudioGraph<Node>, node: NodeId) -> usize {
			let entry = &this.graph[&node];
			let buffers = entry.read_buffers_uncontended();

			let indegree = buffers.incoming.len();
			if entry.indegree.swap(indegree, Relaxed) == usize::MAX {
				if indegree == 0 {
					this.queue.push(node).unwrap();
				}

				1 + buffers
					.incoming
					.keys()
					.map(|&node| visit(this, node))
					.sum::<usize>()
			} else {
				0
			}
		}

		self.curr_len = audio.len();

		for entry in self.graph.values_mut() {
			*entry.indegree.get_mut() = usize::MAX;
		}

		let count = visit(self, node);

		let mut pool = self.pool.take().unwrap();
		pool.run(self, count, NonZero::new(self.queue.len()).unwrap());
		self.pool = Some(pool);

		self.needs_reset = false;

		self.copy_output(node, audio);
	}

	#[expect(clippy::significant_drop_tightening)]
	pub(crate) fn process_node(
		&self,
		entry: &Entry<Node>,
		scratch: &mut [[f32; 2]],
		injector: &Injector<Inject<Node>>,
	) -> Option<NodeId> {
		debug_assert_eq!(entry.indegree.load(Relaxed), 0);

		let mut node = entry.node_uncontended();

		let mut buffers_lock = entry.write_buffers_uncontended();
		let buffers = &mut *buffers_lock;

		if self.needs_reset {
			node.reset();

			for Incoming {
				delay_line, events, ..
			} in buffers.incoming.values_mut()
			{
				delay_line.reset();
				events.clear();
			}
		}

		buffers.audio[..self.curr_len].fill([0.0; 2]);
		buffers.events.clear();

		let max_latency = buffers
			.incoming
			.keys()
			.map(|node| self.graph[node].latency.load(Relaxed))
			.max()
			.unwrap_or_default();

		for (
			dep,
			Incoming {
				delay_line,
				events,
				mix,
			},
		) in &mut buffers.incoming
		{
			let dep_entry = &self.graph[dep];
			let dep_buffers = &*dep_entry.read_buffers_uncontended();
			let latency_diff = max_latency - dep_entry.latency.load(Relaxed);
			delay_line.resize(latency_diff);

			let audio = if latency_diff == 0 {
				&dep_buffers.audio[..self.curr_len]
			} else {
				scratch[..self.curr_len].copy_from_slice(&dep_buffers.audio[..self.curr_len]);
				delay_line.advance(&mut scratch[..self.curr_len]);
				&scratch[..self.curr_len]
			};

			audio
				.as_flattened()
				.iter()
				.zip(buffers.audio[..self.curr_len].as_flattened_mut())
				.for_each(|(&sample, buf)| *buf += *mix * sample);

			events.extend(
				dep_buffers
					.events
					.iter()
					.map(|e| e.at(e.time() + latency_diff)),
			);

			buffers.events.extend(events.extract_if(.., |e| {
				e.time()
					.checked_sub(self.curr_len)
					.map(|time| *e = e.at(time))
					.is_none()
			}));
		}

		entry.latency.store(
			node.process(
				&self.state,
				&mut buffers.audio[..self.curr_len],
				&mut buffers.events,
				injector,
			) + max_latency,
			Relaxed,
		);

		drop(node);

		let buffers_lock = RwLockWriteGuard::downgrade(buffers_lock);

		let mut iter = buffers_lock
			.outgoing
			.iter()
			.copied()
			.filter(|node| self.graph[node].indegree.fetch_sub(1, Relaxed) == 1);

		let inline = iter.next();

		for node in iter {
			self.queue.push(node).unwrap();
		}

		inline
	}

	pub fn reset(&mut self) {
		self.needs_reset = true;
	}

	pub fn state(&self) -> &Node::State {
		&self.state
	}

	pub fn state_mut(&mut self) -> &mut Node::State {
		&mut self.state
	}

	fn entry_mut(&mut self, node: NodeId) -> &mut Entry<Node> {
		self.graph.get_mut(&node).unwrap()
	}

	pub fn for_node_mut(&mut self, node: NodeId, f: impl FnOnce(&mut Node, &Node::State)) {
		f(&mut *self.graph.get_mut(&node).unwrap().node(), &self.state);
	}

	pub fn for_each_node_mut(&mut self, mut f: impl FnMut(&mut Node)) {
		for entry in self.graph.values_mut() {
			f(&mut *entry.node());
		}
	}

	pub fn copy_output(&mut self, node: NodeId, audio: &mut [[f32; 2]]) {
		audio.copy_from_slice(&self.entry_mut(node).buffers().audio[..audio.len()]);
	}

	#[must_use]
	pub fn latency(&self, node: NodeId) -> usize {
		self.graph[&node].latency.load(Relaxed)
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
			self.graph.insert(id, Entry::new(node, self.max_frames));
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
		fn visit<Node: NodeImpl>(this: &AudioGraph<Node>, entry: &Entry<Node>) {
			debug_assert_eq!(entry.indegree.load(Relaxed), usize::MAX);

			entry
				.read_buffers_uncontended()
				.outgoing
				.iter()
				.map(|node| &this.graph[node])
				.filter(|dep| dep.indegree.fetch_sub(1, Relaxed) == 0)
				.for_each(|dep| visit(this, dep));
		}

		for entry in self.graph.values_mut() {
			*entry.indegree.get_mut() = entry.buffers().incoming.len();
		}

		self.graph
			.values()
			.filter(|entry| entry.indegree.fetch_sub(1, Relaxed) == 0)
			.for_each(|entry| visit(self, entry));

		self.graph
			.values_mut()
			.any(|entry| *entry.indegree.get_mut() != usize::MAX)
	}
}
