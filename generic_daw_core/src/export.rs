use crate::{AudioGraph, METER, Position};
use hound::WavWriter;
use std::{path::Path, sync::Arc};

pub fn export(audio_graph: &mut AudioGraph, path: &Path, end: Position) {
    let old_meter = METER.load().clone();

    let mut meter = *old_meter;
    meter.playing = true;
    meter.metronome = true;
    let mut meter = Arc::new(meter);

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
    let end = end.in_samples(meter.bpm, meter.sample_rate) + delay;

    for i in (0..delay - skip).step_by(buffer_size) {
        Arc::get_mut(&mut meter).unwrap().sample = i;
        meter = METER.swap(meter);
        audio_graph.process(&mut buf);
    }

    if skip != 0 {
        Arc::get_mut(&mut meter).unwrap().sample = delay - skip;
        meter = METER.swap(meter);
        audio_graph.process(&mut buf[..skip]);
    }

    let skip = (end - delay) % buffer_size;
    for i in (delay..end - skip).step_by(buffer_size) {
        Arc::get_mut(&mut meter).unwrap().sample = i;
        meter = METER.swap(meter);
        audio_graph.process(&mut buf);

        for &s in &buf {
            writer.write_sample(s).unwrap();
        }
    }

    if skip != 0 {
        Arc::get_mut(&mut meter).unwrap().sample = delay - skip;
        METER.store(meter);
        audio_graph.process(&mut buf[..skip]);

        for &s in &buf[..skip] {
            writer.write_sample(s).unwrap();
        }
    }

    writer.finalize().unwrap();

    METER.store(old_meter);
}
