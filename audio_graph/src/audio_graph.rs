use crate::{EventImpl as _, NodeId, NodeImpl, entry::Entry};
use bit_set::BitSet;
use generic_daw_utils::{HoleyVec, RotateConcatExt as _};

#[derive(Debug)]
pub struct AudioGraph<Node: NodeImpl> {
	graph: HoleyVec<Entry<Node>>,
	root: NodeId,
	list: Vec<usize>,
	swap_list: Vec<usize>,
	to_visit: BitSet,
	seen: BitSet,
}

impl<Node: NodeImpl> AudioGraph<Node> {
	#[must_use]
	pub fn new(node: Node) -> Self {
		let root = node.id();

		let mut graph = HoleyVec::default();
		graph.insert(*root, Entry::new(node));

		Self {
			graph,
			root,
			list: vec![*root],
			swap_list: Vec::new(),
			seen: BitSet::default(),
			to_visit: BitSet::default(),
		}
	}

	pub fn process(&mut self, state: &Node::State, buf: &mut [f32]) {
		for &node in &self.list {
			for s in &mut *buf {
				*s = 0.0;
			}

			let mut entry = self.graph.remove(node).unwrap();

			let max_delay = entry
				.connections
				.keys()
				.map(|node| self.graph[node].delay)
				.max()
				.unwrap_or_default();

			entry.audio.clear();
			entry.audio.resize(buf.len(), 0.0);
			entry.events.clear();

			for (dep, (audio, events)) in entry.connections.iter_mut() {
				let dep_entry = &self.graph[dep];
				let dep_delay = max_delay - dep_entry.delay;

				buf.copy_from_slice(&dep_entry.audio);
				audio.resize(dep_delay, 0.0);
				audio.rotate_right_concat(buf);

				buf.iter()
					.zip(&mut entry.audio)
					.for_each(|(&sample, buf)| *buf += sample);

				events.extend(
					dep_entry
						.events
						.iter()
						.map(|e| e.with_time(e.time() + dep_delay)),
				);

				entry.events.extend(events.extract_if(.., |e| {
					e.time()
						.checked_sub(buf.len())
						.map(|time| {
							*e = e.with_time(time);
						})
						.is_some()
				}));
			}

			entry
				.node
				.process(state, &mut entry.audio, &mut entry.events);

			entry.delay = entry.node.delay() + max_delay;

			self.graph.insert(node, entry);
		}

		buf.copy_from_slice(&self.graph[*self.root()].audio);
	}

	#[must_use]
	pub fn root(&self) -> NodeId {
		self.root
	}

	#[must_use]
	pub fn node(&self, node: NodeId) -> Option<&Node> {
		self.graph.get(*node).map(|entry| &entry.node)
	}

	#[must_use]
	pub fn node_mut(&mut self, node: NodeId) -> Option<&mut Node> {
		self.graph.get_mut(*node).map(|entry| &mut entry.node)
	}

	pub fn reset(&mut self) {
		for entry in self.graph.values_mut() {
			entry.node.reset();
		}
	}

	#[must_use]
	pub fn delay(&self) -> usize {
		self.graph[*self.root()].delay
	}

	#[must_use]
	pub fn connect(&mut self, from: NodeId, to: NodeId) -> bool {
		if !self.graph.contains_key(*to) || !self.graph.contains_key(*from) {
			return false;
		}

		if self
			.graph
			.get_mut(*from)
			.unwrap()
			.connections
			.contains_key(*to)
		{
			return true;
		}

		self.graph
			.get_mut(*from)
			.unwrap()
			.connections
			.insert(*to, (Vec::new(), Vec::new()));

		if self.has_cycle() {
			self.graph.get_mut(*from).unwrap().connections.remove(*to);

			return false;
		}

		true
	}

	pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
		if let Some(entry) = self.graph.get_mut(*from) {
			entry.connections.remove(*to);
		}
	}

	pub fn insert(&mut self, node: Node) {
		let id = node.id();

		if let Some(entry) = self.graph.get_mut(*id) {
			entry.node = node;
			return;
		}

		self.graph.insert(*id, Entry::new(node));
		self.list.push(*id);
	}

	pub fn remove(&mut self, node: NodeId) {
		debug_assert!(node != self.root());

		if self.graph.remove(*node).is_some() {
			let idx = self.list.iter().position(|&n| n == *node).unwrap();
			self.list.remove(idx);

			for entry in self.graph.values_mut() {
				entry.connections.remove(*node);
			}
		}
	}

	fn has_cycle(&mut self) -> bool {
		self.to_visit.clear();
		self.to_visit.extend(self.list.iter().copied());
		self.seen.clear();
		self.swap_list.clear();

		while let Some(node) = self.to_visit.iter().next() {
			if Self::visit(
				&self.graph,
				&mut self.swap_list,
				&mut self.seen,
				&mut self.to_visit,
				node,
			) {
				return true;
			}
		}

		std::mem::swap(&mut self.list, &mut self.swap_list);

		false
	}

	fn visit(
		graph: &HoleyVec<Entry<Node>>,
		list: &mut Vec<usize>,
		seen: &mut BitSet,
		to_visit: &mut BitSet,
		current: usize,
	) -> bool {
		if !to_visit.contains(current) {
			return false;
		}

		if !seen.insert(current) {
			return true;
		}

		if graph[current]
			.connections
			.keys()
			.any(|current| Self::visit(graph, list, seen, to_visit, current))
		{
			return true;
		}

		to_visit.remove(current);
		list.push(current);

		false
	}
}
