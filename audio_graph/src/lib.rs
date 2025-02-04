use std::f32::consts::PI;

mod audio_graph;
mod audio_graph_entry;
mod audio_graph_node;
mod audio_graph_node_impl;
mod mixer_node;
mod node_id;

pub use audio_graph::AudioGraph;
pub use audio_graph_node::AudioGraphNode;
pub use audio_graph_node_impl::AudioGraphNodeImpl;
pub use mixer_node::MixerNode;
pub use node_id::NodeId;

#[must_use]
pub fn pan(angle: f32) -> [f32; 2] {
    let angle = angle.mul_add(0.5, 0.5) * PI * 0.5;

    [angle.cos(), angle.sin()]
}
