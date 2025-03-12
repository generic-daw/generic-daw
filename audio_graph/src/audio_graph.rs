use crate::{AudioGraphNode, NodeId, audio_graph_entry::AudioGraphEntry};
use bit_set::BitSet;
use generic_daw_utils::HoleyVec;
use std::cmp::Ordering;

#[derive(Debug, Default)]
pub struct AudioGraph {
    /// a `NodeId` -> `AudioGraphEntry` map
    graph: HoleyVec<AudioGraphEntry>,
    /// all nodes in the graph in topologically sorted order,
    /// every node comes before all of its dependencies
    list: Vec<NodeId>,
    /// whether `list` needs to be re-sorted before audio processing happens
    dirty: bool,
    /// cache to make cycle checking not allocate on average
    visited: BitSet,
}

impl AudioGraph {
    /// process audio data into `buf`
    ///
    /// `buf` is assumed to be "uninitialized"
    pub fn fill_buf(&mut self, buf: &mut [f32]) {
        if self.dirty {
            self.dirty = false;

            // we need to re-sort our list
            // if there exists a connection `a -> b`, then we treat `a < b`
            // otherwise, we treat `a == b`
            self.list.sort_unstable_by(|&lhs, &rhs| {
                if self.graph[*lhs].connections.contains(*rhs) {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });
        }

        // iterate in reverse to process every node's dependencies before itself
        for node in self.list.iter().copied().rev() {
            let mut deps = self.graph[*node].connections.iter();

            if let Some(dep) = deps.next() {
                // if `node` has dependencies, we don't need to zero `buf`
                // instead, we just copy the cached output of the first dependency we encounter
                buf.copy_from_slice(&self.graph[dep].cache);

                // now we add the cached output of all other dependencies
                for dep in deps {
                    self.graph[dep]
                        .cache
                        .iter()
                        .zip(&mut *buf)
                        .for_each(|(sample, buf)| {
                            *buf += sample;
                        });
                }
            } else {
                // if `node` has no dependencies, zero `buf`
                for s in &mut *buf {
                    *s = 0.0;
                }
            }

            // `buf` now contains exactly the output of all of `node`'s dependencies
            self.graph[*node].node.fill_buf(buf);

            // cache `node`'s output for other nodes that depend on it
            let cache = &mut self.graph.get_mut(*node).unwrap().cache;
            cache.clear();
            cache.extend_from_slice(&*buf);
        }
    }

    /// reset every node in the graph to a pre-playback state
    pub fn reset(&self) {
        for entry in self.graph.values() {
            entry.node.reset();
        }
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

        if self.graph[*from].connections.contains(*to) {
            return true;
        }

        self.visited.clear();
        if Self::check_cycle(&self.graph, &mut self.visited, *to, *from) {
            // if there exists a path from `to` to `from`, connecting `from` to `to` would lead to a cycle
            return false;
        }

        self.graph.get_mut(*from).unwrap().connections.insert(*to);

        if !self.dirty {
            for id in self.list.iter().copied() {
                if id == from {
                    break;
                } else if id == to {
                    // if `to` comes before `from` in our list, it is no longer sorted
                    self.dirty = true;
                    break;
                }
            }
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
    pub fn insert(&mut self, node: AudioGraphNode) {
        let id = node.id();

        if let Some(entry) = self.graph.get_mut(*id) {
            entry.node = node;
            return;
        }

        let entry = AudioGraphEntry {
            node,
            connections: BitSet::new(),
            cache: Vec::new(),
        };

        self.graph.insert(*id, entry);
        // adding a node with no dependencies to the end preserves sorted order
        self.list.push(id);
    }

    /// attempt to remove `node` from the graph
    ///
    /// if the graph contains `node` it is removed along with all adjacent edges,
    /// otherwise this does nothing
    pub fn remove(&mut self, node: NodeId) {
        if self.graph.remove(*node).is_some() {
            let idx = self.list.iter().copied().position(|n| n == node).unwrap();
            // shift-removing a node preserves sorted order
            self.list.remove(idx);

            for entry in self.graph.values_mut() {
                entry.connections.remove(*node);
            }
        }
    }

    /// returns whether there exists a path from `current` to `to`
    ///
    /// this is just a DFS
    fn check_cycle(
        graph: &HoleyVec<AudioGraphEntry>,
        visited: &mut BitSet,
        current: usize,
        to: usize,
    ) -> bool {
        current == to
            || graph[current].connections.iter().any(|current| {
                visited.insert(current) && Self::check_cycle(graph, visited, current, to)
            })
    }
}
