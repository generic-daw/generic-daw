use crate::{AudioGraphNodeImpl, NodeId, audio_graph_entry::AudioGraphEntry};
use bit_set::BitSet;
use generic_daw_utils::{HoleyVec, RotateConcat as _};
use std::ops::AddAssign;

#[derive(Debug)]
pub struct AudioGraph<N, S, E> {
    /// a `NodeId` -> `AudioGraphEntry` map
    graph: HoleyVec<AudioGraphEntry<N, S, E>>,
    /// the `NodeId` of the root node
    root: usize,
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

impl<N, S, E> AudioGraph<N, S, E>
where
    N: AudioGraphNodeImpl<S, E>,
    S: AddAssign + Copy + Default,
    E: Copy,
{
    /// create a new audio graph with the given root node
    #[must_use]
    pub fn new(node: N) -> Self {
        let root = node.id().get();

        let mut graph = HoleyVec::default();
        graph.insert(root, AudioGraphEntry::new(node));

        Self {
            graph,
            root,
            list: vec![root],
            swap_list: Vec::new(),
            seen: BitSet::default(),
            to_visit: BitSet::default(),
        }
    }

    /// process audio data into `buf`
    ///
    /// `buf` is assumed to be "uninitialized"
    pub fn process(&mut self, buf: &mut [S]) {
        for node in self.list.iter().copied() {
            for s in &mut *buf {
                *s = S::default();
            }

            let mut entry = self.graph.remove(node).unwrap();

            let max_delay = entry
                .connections
                .keys()
                .map(|node| self.graph[node].delay)
                .max()
                .unwrap_or_default();

            entry.audio.clear();
            entry.audio.resize(buf.len(), S::default());
            entry.events.clear();

            for (dep, delay) in entry.connections.iter_mut() {
                let dep_entry = &self.graph[dep];

                // apply the needed delay to the audio
                buf.copy_from_slice(&dep_entry.audio);
                delay.resize(max_delay - dep_entry.delay, S::default());
                delay.rotate_right_concat(buf);

                buf.iter()
                    .copied()
                    .zip(&mut entry.audio)
                    .for_each(|(sample, buf)| *buf += sample);

                entry.events.extend(dep_entry.events.iter().copied());
            }

            entry.node.process(&mut entry.audio, &mut entry.events);

            // this node's delay + the largest dependency's delay
            entry.delay = entry.node.delay() + max_delay;

            self.graph.insert(node, entry);
        }

        buf.copy_from_slice(&self.graph[self.root].audio);
    }

    /// reset every node in the graph to a pre-playback state
    pub fn reset(&self) {
        for entry in self.graph.values() {
            entry.node.reset();
        }
    }

    /// get the delay of the entire audio graph
    #[must_use]
    pub fn delay(&self) -> usize {
        self.graph[self.root].delay
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
        let from = from.get();
        let to = to.get();

        debug_assert!(self.root != to);

        if !self.graph.contains_key(to) || !self.graph.contains_key(from) {
            return false;
        }

        if self
            .graph
            .get_mut(from)
            .unwrap()
            .connections
            .contains_key(to)
        {
            return true;
        }

        self.graph
            .get_mut(from)
            .unwrap()
            .connections
            .insert(to, Vec::new());

        if from < to {
            // the old sorted order is still sorted with the new connection
            // since `to` and all of its dependencies come before `from`
            return true;
        }

        if self.has_cycle() {
            self.graph.get_mut(from).unwrap().connections.remove(to);

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
        if let Some(entry) = self.graph.get_mut(from.get()) {
            entry.connections.remove(to.get());
        }
    }

    /// insert `node` into the graph
    ///
    /// if the graph already contains `node` it is replaced, preserving all of its connections,
    /// otherwise it starts out with no connections
    pub fn insert(&mut self, node: N) {
        let id = node.id().get();

        if let Some(entry) = self.graph.get_mut(id) {
            entry.node = node;
            return;
        }

        let entry = AudioGraphEntry::new(node);

        self.graph.insert(id, entry);
        // adding a node with no dependencies to any position preserves sorted order
        // don't just append it, `root` needs to stay at the end
        self.list.insert(self.list.len() - 1, id);
    }

    /// attempt to remove `node` from the graph
    ///
    /// if the graph contains `node` it is removed along with all adjacent edges,
    /// otherwise this does nothing
    pub fn remove(&mut self, node: NodeId) {
        let node = node.get();

        debug_assert!(self.root != node);

        if self.graph.remove(node).is_some() {
            let idx = self.list.iter().copied().position(|n| n == node).unwrap();
            // shift-removing a node preserves sorted order
            self.list.remove(idx);

            for entry in self.graph.values_mut() {
                entry.connections.remove(node);
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

        // `cache` now contains all nodes in reverse topological order
        std::mem::swap(&mut self.list, &mut self.swap_list);

        // shift `root` to the end so we process it last
        let idx = self
            .list
            .iter()
            .copied()
            .position(|id| id == self.root)
            .unwrap();
        self.list[idx..].rotate_left(1);

        false
    }

    /// pushes the nodes in `to_visit` that are in `current`'s directly unvisited subtree into `list` in reverse topological order
    ///
    /// returns whether there is a cycle in `current`'s directly unvisited subtree
    fn visit(
        graph: &HoleyVec<AudioGraphEntry<N, S, E>>,
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

        for current in graph[current].connections.keys() {
            if Self::visit(graph, list, seen, to_visit, current) {
                return true;
            }
        }

        to_visit.remove(current);
        list.push(current);

        false
    }
}
