use crate::{AudioGraph, AudioGraphNode, MusicalTime, daw_ctx::State};
use hound::WavWriter;
use std::path::Path;

#[derive(Debug)]
pub struct Export {
	pub(crate) audio_graph: AudioGraph,
	pub(crate) state: State,
}

impl Export {
	pub fn export(&mut self, path: &Path, len: MusicalTime, mut progress_fn: impl FnMut(f32)) {
		let old = self.state.rtstate;
		self.audio_graph.for_each_node_mut(AudioGraphNode::reset);

		self.state.rtstate.sample = 0;
		self.state.rtstate.playing = true;

		let mut writer = WavWriter::create(
			path,
			hound::WavSpec {
				channels: 2,
				sample_rate: self.state.rtstate.sample_rate.get(),
				bits_per_sample: 32,
				sample_format: hound::SampleFormat::Float,
			},
		)
		.unwrap();

		let buffer_size = 2 * self.state.rtstate.frames.get() as usize;
		let mut buf = vec![0.0; buffer_size].into_boxed_slice();

		let mut delay;
		let mut end;

		while {
			delay = self.audio_graph.delay();
			end = len.to_samples(&self.state.rtstate) + delay;
			self.state.rtstate.sample < delay
		} {
			let diff = buffer_size.min(delay - self.state.rtstate.sample);

			self.audio_graph.process(&self.state, &mut buf[..diff]);
			self.state.updates.get_mut().unwrap().clear();

			self.state.rtstate.sample += diff;
			progress_fn(self.state.rtstate.sample as f32 / end as f32);
		}

		while {
			delay = self.audio_graph.delay();
			end = len.to_samples(&self.state.rtstate) + delay;
			self.state.rtstate.sample < end
		} {
			let diff = buffer_size.min(end - self.state.rtstate.sample);

			self.audio_graph.process(&self.state, &mut buf[..diff]);
			self.state.updates.get_mut().unwrap().clear();

			for &s in &buf[..diff] {
				writer.write_sample(s).unwrap();
			}

			self.state.rtstate.sample += diff;
			progress_fn(self.state.rtstate.sample as f32 / end as f32);
		}

		writer.finalize().unwrap();

		self.state.rtstate = old;
		self.audio_graph.for_each_node_mut(AudioGraphNode::reset);
	}
}
