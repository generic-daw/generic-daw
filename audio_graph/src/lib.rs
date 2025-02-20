use generic_daw_utils::unique_id;
use std::f32::consts::{FRAC_PI_4, SQRT_2};

mod audio_graph;
mod audio_graph_entry;
mod audio_graph_node;
mod audio_graph_node_impl;
mod delay_compensation_node;
mod mixer_node;

pub use audio_graph::AudioGraph;
pub use audio_graph_node::AudioGraphNode;
pub use audio_graph_node_impl::AudioGraphNodeImpl;
pub use delay_compensation_node::DelayCompensationNode;
pub use mixer_node::MixerNode;
pub use node_id::Id as NodeId;

unique_id!(node_id);

#[must_use]
pub fn pan(angle: f32) -> [f32; 2] {
    let angle = (angle + 1.0) * FRAC_PI_4;

    [angle.cos(), angle.sin()].map(|s| s * SQRT_2)
}
