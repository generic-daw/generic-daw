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
	end: MusicalTime,
) {
	export_with(audio_graph, path, rtstate, end, |_| ());
}

pub fn export_with(
	audio_graph: &mut AudioGraph<AudioGraphNode>,
	path: &Path,
	rtstate: RtState,
	end: MusicalTime,
	mut progress_fn: impl FnMut(f32),
) {
	let now = Instant::now();

	let mut state = State {
		rtstate,
		batch: Batch::new(Version::unique()),
	};

	state.rtstate.playing = true;
	state.rtstate.metronome = false;

	audio_graph.for_each_mut(AudioGraphNode::reset);

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

	let delay = audio_graph.delay();
	let skip = delay % buffer_size;
	let end = end.to_samples(&rtstate);

	for i in (0..delay).step_by(buffer_size) {
		state.rtstate.sample = i;

		audio_graph.process(&mut state, &mut buf);
		state.batch.updates.clear();
	}

	if skip != 0 {
		state.rtstate.sample = delay - skip;

		audio_graph.process(&mut state, &mut buf[..skip]);
		state.batch.updates.clear();
	}

	for i in (delay..end + delay).step_by(buffer_size) {
		state.rtstate.sample = i;

		audio_graph.process(&mut state, &mut buf);
		state.batch.updates.clear();

		for &s in &buf {
			writer.write_sample(s).unwrap();
		}

		progress_fn((state.rtstate.sample - delay) as f32 / end as f32);
	}

	writer.finalize().unwrap();

	info!(
		"export of {} finished in {:?}",
		path.display(),
		now.elapsed()
	);
}
