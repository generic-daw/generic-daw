use crate::{AudioGraphNode, Meter, Position, daw_ctx::State};
use audio_graph::AudioGraph;
use hound::WavWriter;
use std::path::Path;

pub fn export(
    audio_graph: &mut AudioGraph<AudioGraphNode>,
    path: &Path,
    meter: Meter,
    end: Position,
) {
    let mut state = State {
        meter,
        sender: async_channel::unbounded().0,
        receiver: async_channel::unbounded().1,
    };

    state.meter.playing = true;
    state.meter.metronome = false;

    audio_graph.reset();

    let mut writer = WavWriter::create(
        path,
        hound::WavSpec {
            channels: 2,
            sample_rate: meter.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();

    let buffer_size = meter.buffer_size as usize;
    let mut buf = vec![0.0; buffer_size].into_boxed_slice();

    let delay = audio_graph.delay();
    let skip = delay % buffer_size;
    let end = end.in_samples(&meter);

    for i in (0..delay).step_by(buffer_size) {
        state.meter.sample = i;

        audio_graph.process(&state, &mut buf);
    }

    if skip != 0 {
        state.meter.sample = delay - skip;

        audio_graph.process(&state, &mut buf[..skip]);
    }

    for i in (delay..end + delay).step_by(buffer_size) {
        state.meter.sample = i;

        audio_graph.process(&state, &mut buf);

        for &s in &buf {
            writer.write_sample(s).unwrap();
        }
    }

    if skip != 0 {
        state.meter.sample = end + delay - skip;

        audio_graph.process(&state, &mut buf[..skip]);

        for &s in &buf[..skip] {
            writer.write_sample(s).unwrap();
        }
    }

    writer.finalize().unwrap();
}
