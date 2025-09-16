use crate::{
	AudioGraphNode, MusicalTime, RtState, Version,
	daw_ctx::{Batch, State},
};
use audio_graph::AudioGraph;
use hound::WavWriter;
use log::info;
use std::{path::Path, time::Instant};

pub fn export(
	audio_graph: &mut AudioGraph<AudioGraphNode>,
	path: &Path,
	rtstate: RtState,
	len: MusicalTime,
) {
	export_with(audio_graph, path, rtstate, len, |_| ());
}

pub fn export_with(
	audio_graph: &mut AudioGraph<AudioGraphNode>,
	path: &Path,
	mut rtstate: RtState,
	len: MusicalTime,
	mut progress_fn: impl FnMut(f32),
) {
	let now = Instant::now();

	rtstate.sample = 0;
	rtstate.playing = true;
	rtstate.metronome = false;

	audio_graph.for_each_mut(AudioGraphNode::reset);

	let mut state = State {
		rtstate,
		batch: Batch::new(Version::unique()),
	};

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

	let buffer_size = 2 * rtstate.frames as usize;
	let mut buf = vec![0.0; buffer_size].into_boxed_slice();

	let delay = audio_graph.delay().next_multiple_of(2);
	let end = len.to_samples(&rtstate).next_multiple_of(2) + delay;

	while state.rtstate.sample < delay {
		let diff = buffer_size.min(delay - state.rtstate.sample);
		state.rtstate.sample += diff;

		audio_graph.process(&mut state, &mut buf[..diff]);
		state.batch.updates.clear();

		progress_fn(state.rtstate.sample as f32 / end as f32);
	}

	while state.rtstate.sample < end {
		let diff = buffer_size.min(end - state.rtstate.sample);
		state.rtstate.sample += diff;

		audio_graph.process(&mut state, &mut buf[..diff]);
		state.batch.updates.clear();

		for &s in &buf[..diff] {
			writer.write_sample(-s).unwrap();
		}

		progress_fn(state.rtstate.sample as f32 / end as f32);
	}

	writer.finalize().unwrap();

	info!(
		"export of {} finished in {:?}",
		path.display(),
		now.elapsed()
	);
}
