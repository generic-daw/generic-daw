use crate::{AudioGraphNode, MusicalTime, daw_ctx::State};
use audio_graph::AudioGraph;
use hound::WavWriter;
use log::info;
use std::{path::Path, time::Instant};

pub fn export(
	audio_graph: &mut AudioGraph<AudioGraphNode>,
	path: &Path,
	state: impl Into<State>,
	len: MusicalTime,
) {
	export_with(audio_graph, path, state, len, |_| ());
}

pub fn export_with(
	audio_graph: &mut AudioGraph<AudioGraphNode>,
	path: &Path,
	state: impl Into<State>,
	len: MusicalTime,
	mut progress_fn: impl FnMut(f32),
) {
	let now = Instant::now();

	let mut state = state.into();
	state.rtstate.playing = true;
	state.rtstate.metronome = false;

	let mut writer = WavWriter::create(
		path,
		hound::WavSpec {
			channels: 2,
			sample_rate: state.rtstate.sample_rate,
			bits_per_sample: 32,
			sample_format: hound::SampleFormat::Float,
		},
	)
	.unwrap();

	let buffer_size = 2 * state.rtstate.frames as usize;
	let mut buf = vec![0.0; buffer_size].into_boxed_slice();

	let mut delay;
	let mut end;

	audio_graph.for_each_mut(AudioGraphNode::reset);

	while {
		delay = audio_graph.delay().next_multiple_of(2);
		end = len.to_samples(&state.rtstate).next_multiple_of(2) + delay;
		state.rtstate.sample < delay
	} {
		let diff = buffer_size.min(delay - state.rtstate.sample);
		state.rtstate.sample += diff;

		audio_graph.process(&mut state, &mut buf[..diff]);
		state.batch.updates.clear();

		progress_fn(state.rtstate.sample as f32 / end as f32);
	}

	while {
		delay = audio_graph.delay().next_multiple_of(2);
		end = len.to_samples(&state.rtstate).next_multiple_of(2) + delay;
		state.rtstate.sample < end
	} {
		let diff = buffer_size.min(end - state.rtstate.sample);
		state.rtstate.sample += diff;

		audio_graph.process(&mut state, &mut buf[..diff]);
		state.batch.updates.clear();

		for &s in &buf[..diff] {
			writer.write_sample(s).unwrap();
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
