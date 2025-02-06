use crate::{audio_graph_entry::AudioGraphEntry, node_id::NodeId, AudioGraphNode};
use gxhash::{HashMap, HashSet, HashSetExt as _};
use std::cmp::Ordering;

#[derive(Debug, Default)]
pub struct AudioGraph {
    graph: HashMap<NodeId, AudioGraphEntry>,
    dep_list: Vec<NodeId>,
    dirty: bool,
}

impl AudioGraph {
    #[must_use]
    pub fn new(node: AudioGraphNode) -> Self {
        let id = node.id();
        let entry = AudioGraphEntry {
            node,
            connections: HashSet::default(),
            cache: Vec::new(),
        };

        Self {
            graph: HashMap::from_iter([(id, entry)]),
            dep_list: vec![id],
            dirty: false,
        }
    }

    pub fn fill_buf(&mut self, buf_start_sample: usize, buf: &mut [f32]) {
        if self.dirty {
            self.dirty = false;

            self.dep_list.sort_unstable_by(|lhs, rhs| {
                if self.graph[lhs].connections.contains(rhs) {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });

            debug_assert_eq!(usize::from(self.dep_list[0]), 0);
        }

        for node in self.dep_list.iter().rev() {
            for s in buf.iter_mut() {
                *s = 0.0;
            }

            for node in &self.graph[node].connections {
                self.graph[node]
                    .cache
                    .iter()
                    .zip(buf.iter_mut())
                    .for_each(|(sample, buf)| {
                        *buf += sample;
                    });
            }

            self.graph[node].node.fill_buf(buf_start_sample, buf);

            let cbuf = &mut self.graph.get_mut(node).unwrap().cache;
            cbuf.clear();
            cbuf.extend(&*buf);
        }
    }

    pub fn connect(&mut self, from: NodeId, to: NodeId) -> bool {
        if self.graph.contains_key(&to)
            && self
                .graph
                .get(&from)
                .is_some_and(|g| !g.connections.contains(&to))
            && !self.check_cycle(&mut HashSet::with_capacity(self.graph.len()), to, from)
        {
            self.graph.get_mut(&from).unwrap().connections.insert(to);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn disconnect(&mut self, from: NodeId, to: NodeId) -> bool {
        self.graph
            .get_mut(&from)
            .is_some_and(|g| g.connections.remove(&to))
    }

    pub fn insert(&mut self, node: AudioGraphNode) {
        let id = node.id();

        let entry = AudioGraphEntry {
            node,
            connections: HashSet::default(),
            cache: Vec::new(),
        };

        self.graph.insert(id, entry);
        self.dep_list.push(id);
    }

    #[must_use]
    pub fn root(&self) -> NodeId {
        self.dep_list[0]
    }

    #[must_use]
    pub fn remove(&mut self, node: NodeId) -> bool {
        debug_assert_ne!(self.dep_list[0], node);

        if self.graph.remove(&node).is_some() {
            self.graph.remove(&node);
            self.dep_list
                .remove(self.dep_list.iter().position(|&n| n == node).unwrap());

            for g in self.graph.values_mut() {
                g.connections.remove(&node);
            }

            true
        } else {
            false
        }
    }

    fn check_cycle(&self, visited: &mut HashSet<NodeId>, current: NodeId, to: NodeId) -> bool {
        if current == to {
            return true;
        }

        for current in &self.graph[&current].connections {
            if visited.contains(current) {
                continue;
            }

            visited.insert(*current);

            if self.check_cycle(visited, *current, to) {
                return true;
            }
        }

        false
    }
}
