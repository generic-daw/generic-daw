struct Sample {
    @location(0) x: f32,
    @location(1) y: f32,
};

struct VertexOutput {
    @builtin(position) p: vec4<f32>,
}

@vertex
fn vs_main(point: Sample) -> VertexOutput {
    var v: VertexOutput;
    v.p = vec4<f32>(point.x, point.y, 0.0, 0.0);
    return v;
}

@group(0) @binding(0)
var s_texture: texture_1d<f32>;
@group(0) @binding(1)
var s_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var sample = textureSample(s_texture, s_sampler, in.p.x);

    if (sample.r <= in.p.y && sample.g >= in.p.y) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    } else {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
}
