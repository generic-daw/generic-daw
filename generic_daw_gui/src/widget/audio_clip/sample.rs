use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Sample(pub f32, pub f32);
