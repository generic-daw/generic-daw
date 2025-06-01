use crate::{EventImpl as _, NodeId, NodeImpl, entry::Entry};
use bit_set::BitSet;
use generic_daw_utils::{HoleyVec, RotateConcatExt as _};

#[derive(Debug)]
pub struct AudioGraph<Node: NodeImpl> {
    /// a `NodeId` -> `Entry` map
    graph: HoleyVec<Entry<Node>>,
    /// the `NodeId` of the root node
    root: NodeId,
    /// all nodes in the graph in reverse topological order,
    /// every node comes after all of its dependencies
    list: Vec<usize>,
    /// cache for cycle checking
    swap_list: Vec<usize>,
    /// cache for cycle checking
    to_visit: BitSet,
    /// cache for cycle checking
    seen: BitSet,
}

impl<Node: NodeImpl> AudioGraph<Node> {
    /// create a new audio graph with the given root node
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

    /// apply `action` to `node`
    ///
    /// this does nothing if the graph doesn't contain `node`
    pub fn apply(&mut self, node: NodeId, action: Node::Action) {
        if let Some(entry) = self.graph.get_mut(*node) {
            entry.node.apply(action);
        }
    }

    /// process audio data into `buf`
    ///
    /// `buf` is assumed to be "uninitialized"
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

                // apply delay to audio
                buf.copy_from_slice(&dep_entry.audio);
                audio.resize(dep_delay, 0.0);
                audio.rotate_right_concat(buf);

                buf.iter()
                    .zip(&mut entry.audio)
                    .for_each(|(&sample, buf)| *buf += sample);

                // apply delay to events
                events.extend(
                    dep_entry
                        .events
                        .iter()
                        .map(|e| e.with_time(e.time() + dep_delay)),
                );

                entry.events.extend(events.extract_if(.., |e| {
                    e.time()
                        .checked_sub(buf.len())
                        .inspect(|&time| {
                            *e = e.with_time(time);
                        })
                        .is_some()
                }));
            }

            entry
                .node
                .process(state, &mut entry.audio, &mut entry.events);

            // this node's delay + the largest dependency's delay
            entry.delay = entry.node.delay() + max_delay;

            self.graph.insert(node, entry);
        }

        buf.copy_from_slice(&self.graph[*self.root].audio);
    }

    /// reset every node in the graph to a pre-playback state
    pub fn reset(&mut self) {
        for entry in self.graph.values_mut() {
            entry.node.reset();
        }
    }

    /// get the delay of the entire audio graph
    #[must_use]
    pub fn delay(&self) -> usize {
        self.graph[*self.root].delay
    }

    /// attempt to connect `from` to `to`,
    /// which signifies that `from` depends on `to`,
    /// or that audio data flows from `to` to `from`
    ///
    /// returns whether the attempt was successful
    ///
    /// an attempt can fail if:
    ///  - the graph doesn't contain `from`
    ///  - the graph doesn't contain `to`
    ///  - connecting `from` to `to` would produce a cycle
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

    /// attempt to disconnect `from` from `to`
    ///
    /// this does nothing if:
    ///  - the graph doesn't contain `from`
    ///  - the graph doesn't contain `to`
    ///  - `from` isn't connected to `to`
    pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
        if let Some(entry) = self.graph.get_mut(*from) {
            entry.connections.remove(*to);
        }
    }

    /// insert `node` into the graph
    ///
    /// if the graph already contains `node` it is replaced, preserving all of its connections,
    /// otherwise it starts out with no connections
    pub fn insert(&mut self, node: Node) {
        let id = node.id();

        if let Some(entry) = self.graph.get_mut(*id) {
            entry.node = node;
            return;
        }

        self.graph.insert(*id, Entry::new(node));
        // adding a node with no dependencies to any position preserves sorted order
        self.list.push(*id);
    }

    /// attempt to remove `node` from the graph
    ///
    /// if the graph contains `node` it is removed along with all adjacent edges,
    /// otherwise this does nothing
    pub fn remove(&mut self, node: NodeId) {
        debug_assert!(self.root != node);

        if self.graph.remove(*node).is_some() {
            let idx = self.list.iter().position(|&n| n == *node).unwrap();
            // shift-removing a node preserves sorted order
            self.list.remove(idx);

            for entry in self.graph.values_mut() {
                entry.connections.remove(*node);
            }
        }
    }

    /// returns whether there is a cycle in the graph
    ///
    /// if there is no cycle, it re-sorts `list`
    fn has_cycle(&mut self) -> bool {
        // save all nodes in `to_visit`
        self.to_visit.clear();
        self.to_visit.extend(self.list.iter().copied());
        self.seen.clear();
        self.swap_list.clear();

        // process one subtree at a time until there are no more nodes left in `to_visit` or a cycle was found
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

        // `swap_list` now contains all nodes in reverse topological order
        std::mem::swap(&mut self.list, &mut self.swap_list);

        false
    }

    /// pushes the nodes in `to_visit` that are in `current`'s directly unvisited subtree into `list` in reverse topological order
    ///
    /// returns whether there is a cycle in `current`'s directly unvisited subtree
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
