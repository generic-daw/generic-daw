use crate::{EventImpl as _, NodeId, NodeImpl, entry::Entry};
use generic_daw_utils::{DelayLine, HoleyVec};
use std::{num::NonZero, sync::atomic::Ordering::Relaxed};

#[derive(Debug)]
pub struct AudioGraph<Node: NodeImpl> {
	graph: HoleyVec<Entry<Node>>,
	root: NodeId,
	frames: NonZero<u32>,
}

impl<Node: NodeImpl> AudioGraph<Node> {
	#[must_use]
	pub fn new(node: impl Into<Node>, frames: NonZero<u32>) -> Self {
		let node = node.into();
		let root = node.id();

		let mut graph = HoleyVec::default();
		graph.insert(*root, Entry::new(node, frames));

		Self {
			graph,
			root,
			frames,
		}
	}

	pub fn process(&mut self, state: &Node::State, buf: &mut [f32]) {
		let len = buf.len();

		for entry in self.graph.values_mut() {
			*entry.indegree.get_mut() = entry.buffers().incoming.len().cast_signed();
			entry.expensive = entry.node().expensive();
		}

		rayon_core::in_place_scope(|s| {
			let mut first = None;

			self.graph
				.values()
				.filter(|entry| entry.indegree.load(Relaxed) == 0)
				.filter(|entry| entry.indegree.fetch_sub(1, Relaxed) == 0)
				.for_each(|entry| {
					if entry.expensive {
						if first.is_none() {
							first = Some(entry);
						} else {
							s.spawn(|s| {
								self.worker(s, entry, len, state);
							});
						}
					} else {
						self.worker(s, entry, len, state);
					}
				});

			if let Some(entry) = first {
				self.worker(s, entry, len, state);
			}
		});

		debug_assert!(
			self.graph
				.values_mut()
				.all(|entry| entry.indegree.get_mut().is_negative())
		);

		buf.copy_from_slice(&self.entry_mut(self.root()).buffers().audio[..len]);
	}

	fn worker<'a>(
		&'a self,
		s: &rayon_core::Scope<'a>,
		entry: &Entry<Node>,
		len: usize,
		state: &'a Node::State,
	) {
		debug_assert!(entry.indegree.load(Relaxed).is_negative());

		let mut node_lock = entry.node_uncontended();
		let mut buffers_lock = entry.write_buffers_uncontended();

		let node = &mut *node_lock;
		let buffers = &mut *buffers_lock;

		buffers.audio[..len].fill(0.0);
		buffers.events.clear();

		let max_delay = buffers
			.incoming
			.keys()
			.map(|node| self.graph[node].delay.load(Relaxed))
			.max()
			.unwrap_or_default();

		for (dep, (buf, events)) in buffers.incoming.iter_mut() {
			let dep_entry = &self.graph[dep];
			let dep_buffers = dep_entry.read_buffers_uncontended();
			let delay_diff = max_delay - dep_entry.delay.load(Relaxed);

			buffers.buf[..len].copy_from_slice(&dep_buffers.audio[..len]);
			buf.resize(delay_diff);
			buf.advance(&mut buffers.buf[..len]);

			buffers.buf[..len]
				.iter()
				.zip(&mut buffers.audio[..len])
				.for_each(|(&sample, buf)| *buf += sample);

			events.extend(
				dep_buffers
					.events
					.iter()
					.map(|e| e.with_time(e.time() + delay_diff)),
			);

			drop(dep_buffers);

			buffers.events.extend(events.extract_if(.., |e| {
				e.time()
					.checked_sub(len)
					.map(|time| {
						*e = e.with_time(time);
					})
					.is_none()
			}));
		}

		node.process(state, &mut buffers.audio[..len], &mut buffers.events);
		entry.delay.store(node.delay() + max_delay, Relaxed);

		drop(node_lock);
		drop(buffers_lock);

		let mut first = None;

		entry
			.read_buffers_uncontended()
			.outgoing
			.iter()
			.map(|node| &self.graph[node])
			.filter(|dep| dep.indegree.fetch_sub(1, Relaxed) == 1)
			.filter(|dep| dep.indegree.fetch_sub(1, Relaxed) == 0)
			.for_each(|dep| {
				if dep.expensive {
					if entry.expensive && first.is_none() {
						first = Some(dep);
					} else {
						s.spawn(move |s| {
							self.worker(s, dep, len, state);
						});
					}
				} else {
					self.worker(s, dep, len, state);
				}
			});

		if let Some(entry) = first {
			self.worker(s, entry, len, state);
		}
	}

	#[must_use]
	pub fn root(&self) -> NodeId {
		self.root
	}

	fn entry_mut(&mut self, node: NodeId) -> &mut Entry<Node> {
		self.graph.get_mut(*node).unwrap()
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
		self.graph[*self.root()].delay.load(Relaxed)
	}

	#[must_use]
	pub fn connect(&mut self, from: NodeId, to: NodeId) -> bool {
		if !self.graph.contains_key(*from) || !self.graph.contains_key(*to) {
			return false;
		}

		if !self.entry_mut(from).buffers().outgoing.insert(*to) {
			return true;
		}

		self.entry_mut(to)
			.buffers()
			.incoming
			.insert(*from, (DelayLine::default(), Vec::new()));

		if self.has_cycle() {
			self.entry_mut(from).buffers().outgoing.remove(*to);
			self.entry_mut(to).buffers().incoming.remove(*from);
			return false;
		}

		true
	}

	pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
		self.entry_mut(from).buffers().outgoing.remove(*to);
		self.entry_mut(to).buffers().incoming.remove(*from);
	}

	pub fn insert(&mut self, node: Node) {
		let id = node.id();

		if let Some(entry) = self.graph.get_mut(*id) {
			*entry.node() = node;
		} else {
			self.graph.insert(*id, Entry::new(node, self.frames));
		}
	}

	pub fn remove(&mut self, node: NodeId) {
		if let Some(mut entry) = self.graph.remove(*node) {
			for incoming in entry.buffers().incoming.keys() {
				self.graph
					.get_mut(incoming)
					.unwrap()
					.buffers()
					.outgoing
					.remove(*node);
			}

			for outgoing in &entry.buffers().outgoing {
				self.graph
					.get_mut(outgoing)
					.unwrap()
					.buffers()
					.incoming
					.remove(*node);
			}
		}
	}

	fn has_cycle(&mut self) -> bool {
		for entry in self.graph.values_mut() {
			*entry.indegree.get_mut() = entry.buffers().incoming.len().cast_signed();
		}

		self.graph
			.values()
			.filter(|entry| entry.indegree.load(Relaxed) == 0)
			.filter(|entry| entry.indegree.fetch_sub(1, Relaxed) == 0)
			.for_each(|entry| self.visit(entry));

		!self
			.graph
			.values_mut()
			.all(|entry| entry.indegree.get_mut().is_negative())
	}

	fn visit(&self, entry: &Entry<Node>) {
		entry
			.read_buffers_uncontended()
			.outgoing
			.iter()
			.map(|node| &self.graph[node])
			.filter(|dep| dep.indegree.fetch_sub(1, Relaxed) == 1)
			.filter(|dep| dep.indegree.fetch_sub(1, Relaxed) == 0)
			.for_each(|dep| self.visit(dep));
	}
}
