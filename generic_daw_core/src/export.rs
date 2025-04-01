use crate::{AudioGraph, Meter, Position};
use hound::WavWriter;
use std::{
    path::Path,
    sync::atomic::Ordering::{AcqRel, Acquire, Release},
};

pub fn export(audio_graph: &mut AudioGraph, path: &Path, meter: &Meter, end: Position) {
    let playing = meter.playing.swap(true, AcqRel);
    let metronome = meter.metronome.swap(false, AcqRel);
    let sample = meter.sample.load(Acquire);

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
    let end = end.in_samples(meter.bpm.load(Acquire), meter.sample_rate) + delay;

    for i in (0..delay - skip).step_by(buffer_size) {
        meter.sample.store(i, Release);

        audio_graph.process(&mut buf);
    }

    if skip != 0 {
        meter.sample.store(delay - skip, Release);

        audio_graph.process(&mut buf[..skip]);
    }

    let skip = (end - delay) % buffer_size;
    for i in (delay..end - skip).step_by(buffer_size) {
        meter.sample.store(i, Release);

        audio_graph.process(&mut buf);

        for &s in &buf {
            writer.write_sample(s).unwrap();
        }
    }

    if skip != 0 {
        meter.sample.store(delay - skip, Release);

        audio_graph.process(&mut buf[..skip]);

        for &s in &buf[..skip] {
            writer.write_sample(s).unwrap();
        }
    }

    writer.finalize().unwrap();

    meter.playing.store(playing, Release);
    meter.metronome.store(metronome, Release);
    meter.sample.store(sample, Release);
}
