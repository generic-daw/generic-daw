use crate::{AudioGraphNode, MusicalTime, RtState, daw_ctx::State};
use audio_graph::AudioGraph;
use hound::WavWriter;
use std::path::Path;

pub fn export(
    audio_graph: &mut AudioGraph<AudioGraphNode>,
    path: &Path,
    rtstate: RtState,
    end: MusicalTime,
) {
    let mut state = State {
        rtstate,
        sender: async_channel::unbounded().0,
        receiver: async_channel::unbounded().1,
    };

    state.rtstate.playing = true;
    state.rtstate.metronome = false;

    audio_graph.reset();

    let mut writer = WavWriter::create(
        path,
        hound::WavSpec {
            channels: 2,
            sample_rate: rtstate.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();

    let buffer_size = rtstate.buffer_size as usize;
    let mut buf = vec![0.0; buffer_size].into_boxed_slice();

    let delay = audio_graph.delay();
    let skip = delay % buffer_size;
    let end = end.to_samples(&rtstate);

    for i in (0..delay).step_by(buffer_size) {
        state.rtstate.sample = i;

        audio_graph.process(&state, &mut buf);
    }

    if skip != 0 {
        state.rtstate.sample = delay - skip;

        audio_graph.process(&state, &mut buf[..skip]);
    }

    for i in (delay..end + delay).step_by(buffer_size) {
        state.rtstate.sample = i;

        audio_graph.process(&state, &mut buf);

        for &s in &buf {
            writer.write_sample(s).unwrap();
        }
    }

    if skip != 0 {
        state.rtstate.sample = end + delay - skip;

        audio_graph.process(&state, &mut buf[..skip]);

        for &s in &buf[..skip] {
            writer.write_sample(s).unwrap();
        }
    }

    writer.finalize().unwrap();
}
