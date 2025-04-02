use generic_daw_utils::unique_id;

mod audio_graph;
mod entry;
mod event_impl;
mod node_impl;

pub use audio_graph::AudioGraph;
pub use event_impl::EventImpl;
pub use node_id::Id as NodeId;
pub use node_impl::NodeImpl;

unique_id!(node_id);
