use generic_daw_utils::unique_id;

mod audio_graph;
mod audio_graph_entry;
mod audio_graph_node_impl;

pub use audio_graph::AudioGraph;
pub use audio_graph_node_impl::AudioGraphNodeImpl;
pub use node_id::Id as NodeId;

unique_id!(node_id);
