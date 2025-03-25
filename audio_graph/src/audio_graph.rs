use crate::{AudioGraphNode, NodeId, audio_graph_entry::AudioGraphEntry};
use bit_set::BitSet;
use generic_daw_utils::{HoleyVec, RotateConcat as _};
use std::sync::{Arc, LazyLock};
use tokio::{
    runtime::Runtime,
    sync::{Barrier, RwLock},
    task::{JoinSet, block_in_place},
};

static EXECUTOR: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());

#[derive(Debug)]
pub struct AudioGraph {
    /// a `NodeId` -> `AudioGraphEntry` map
    graph: HoleyVec<RwLock<AudioGraphEntry>>,
    /// the `NodeId` of the root node
    root: usize,
    /// cache for cycle checking
    to_visit: BitSet,
    /// cache for cycle checking
    seen: BitSet,
}

impl AudioGraph {
    /// create a new audio graph with the given root node
    #[must_use]
    pub fn new(node: AudioGraphNode) -> Self {
        let root = node.id().get();

        let mut graph = HoleyVec::default();
        graph.insert(
            root,
            RwLock::new(AudioGraphEntry {
                node,
                connections: HoleyVec::default(),
                buf: Vec::new(),
                cache: Vec::new(),
                delay: 0,
            }),
        );

        Self {
            graph,
            root,
            seen: BitSet::default(),
            to_visit: BitSet::default(),
        }
    }

    /// process audio data into `buf`
    ///
    /// `buf` is assumed to be "uninitialized"
    pub fn fill_buf(&mut self, buf: &mut [f32]) {
        let graph = Arc::new(std::mem::take(&mut self.graph));

        EXECUTOR.block_on(async {
            let mut join_set = JoinSet::new();
            let barrier = Arc::new(Barrier::new(graph.keys().count()));

            for node in graph.keys() {
                join_set.spawn(Self::worker(
                    graph.clone(),
                    node,
                    buf.len(),
                    barrier.clone(),
                ));
            }

            join_set.join_all().await;
        });

        // there are no weak references, so the strong count is still 1
        self.graph = Arc::into_inner(graph).unwrap();

        buf.copy_from_slice(
            &self.graph[self.root]
                .try_read()
                .expect("this is only locked from the audio thread")
                .cache,
        );
    }

    async fn worker(
        graph: Arc<HoleyVec<RwLock<AudioGraphEntry>>>,
        node: usize,
        buf_len: usize,
        barrier: Arc<Barrier>,
    ) {
        let entry = &mut *graph[node].write().await;

        // wait until all nodes have taken their write lock
        barrier.wait().await;

        entry.buf.clear();
        entry.buf.resize(buf_len, 0.0);
        entry.cache.clear();

        let mut max_delay = 0;
        for dep in entry.connections.keys() {
            // wait until the dependency's future has finished processing
            // we know this because it drops its write lock, and we can read
            //
            // this can't deadlock, because we know that none of this node's
            // dependencies depend on it directly or indirectly
            max_delay = graph[dep].read().await.delay.max(max_delay);
        }

        block_in_place(|| {
            for (dep, cache) in entry.connections.iter_mut() {
                // copy the dependency's cache into our own cache
                let dep = graph[dep].try_read().unwrap();
                entry.cache.extend_from_slice(&dep.cache);
                cache.resize(max_delay - dep.delay, 0.0);
                drop(dep);

                // apply the delay needed to make all dependencies be time-aligned
                cache.rotate_right_concat(&mut entry.cache);

                // copy the delayed audio into `buf`
                entry
                    .cache
                    .drain(..)
                    .zip(&mut *entry.buf)
                    .for_each(|(sample, buf)| *buf += sample);
            }

            // `buf` now contains exactly the output of all of `node`'s time-aligned dependencies
            entry.node.fill_buf(&mut entry.buf);

            // cache `node`'s output for other nodes that depend on it
            entry.cache.extend_from_slice(&entry.buf);

            // this node's delay + the largest dependency's delay
            entry.delay = entry.node.delay() + max_delay;
        });
    }

    /// reset every node in the graph to a pre-playback state
    pub fn reset(&self) {
        for entry in self.graph.values() {
            entry
                .try_read()
                .expect("this is only locked from the audio thread")
                .node
                .reset();
        }
    }

    /// get the delay of the entire audio graph
    #[must_use]
    pub fn delay(&self) -> usize {
        self.graph[self.root]
            .try_read()
            .expect("this is only locked from the audio thread")
            .delay
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

        if self.graph[from]
            .try_read()
            .expect("this is only locked from the audio thread")
            .connections
            .contains_key(to)
        {
            return true;
        }

        self.graph[from]
            .try_write()
            .expect("this is only locked from the audio thread")
            .connections
            .insert(to, Vec::new());

        if from < to {
            // the old sorted order is still sorted with the new connection
            // since `to` and all of its dependencies come before `from`
            return true;
        }

        if self.has_cycle() {
            self.graph[from]
                .try_write()
                .expect("this is only locked from the audio thread")
                .connections
                .remove(to);

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
    pub fn disconnect(&self, from: NodeId, to: NodeId) {
        if let Some(entry) = self.graph.get(from.get()) {
            entry
                .try_write()
                .expect("this is only locked from the audio thread")
                .connections
                .remove(to.get());
        }
    }

    /// insert `node` into the graph
    ///
    /// if the graph already contains `node` it is replaced, preserving all of its connections,
    /// otherwise it starts out with no connections
    pub fn insert(&mut self, node: AudioGraphNode) {
        let id = node.id().get();

        if let Some(entry) = self.graph.get_mut(id) {
            entry
                .try_write()
                .expect("this is only locked from the audio thread")
                .node = node;
            return;
        }

        let entry = RwLock::new(AudioGraphEntry {
            node,
            connections: HoleyVec::default(),
            buf: Vec::new(),
            cache: Vec::new(),
            delay: 0,
        });

        self.graph.insert(id, entry);
    }

    /// attempt to remove `node` from the graph
    ///
    /// if the graph contains `node` it is removed along with all adjacent edges,
    /// otherwise this does nothing
    pub fn remove(&mut self, node: NodeId) {
        let node = node.get();

        debug_assert!(self.root != node);

        if self.graph.remove(node).is_some() {
            for entry in self.graph.values_mut() {
                entry
                    .try_write()
                    .expect("this is only locked from the audio thread")
                    .connections
                    .remove(node);
            }
        }
    }

    /// returns whether there is a cycle in the graph
    ///
    /// if there is no cycle, it re-sorts `list`
    fn has_cycle(&mut self) -> bool {
        // save all nodes in `to_visit`
        self.to_visit.clear();
        self.to_visit.extend(self.graph.keys());
        self.seen.clear();

        // process one subtree at a time until there are no more nodes left in `to_visit` or a cycle was found
        while let Some(node) = self.to_visit.iter().next() {
            if Self::visit(&self.graph, &mut self.seen, &mut self.to_visit, node) {
                return true;
            }
        }

        false
    }

    /// pushes the nodes in `to_visit` that are in `current`'s directly unvisited subtree into `list` in reverse topological order
    ///
    /// returns whether there is a cycle in `current`'s directly unvisited subtree
    fn visit(
        graph: &HoleyVec<RwLock<AudioGraphEntry>>,
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

        for current in graph[current]
            .try_read()
            .expect("this is only locked from the audio thread")
            .connections
            .keys()
        {
            if Self::visit(graph, seen, to_visit, current) {
                return true;
            }
        }

        to_visit.remove(current);

        false
    }
}
