use crate::{AudioGraph, AudioGraphNode, MusicalTime, daw_ctx::State};
use hound::WavWriter;
use std::path::Path;
use utils::boxed_slice;

#[derive(Debug)]
pub struct Export {
	pub(crate) audio_graph: AudioGraph,
	pub(crate) state: State,
}

impl Export {
	pub fn export(&mut self, path: &Path, len: MusicalTime, mut progress_fn: impl FnMut(f32)) {
		let old = self.state.transport;
		self.audio_graph.for_each_node_mut(AudioGraphNode::reset);

		self.state.transport.sample = 0;
		self.state.transport.playing = true;

		let mut writer = WavWriter::create(
			path,
			hound::WavSpec {
				channels: 2,
				sample_rate: self.state.transport.sample_rate.get(),
				bits_per_sample: 32,
				sample_format: hound::SampleFormat::Float,
			},
		)
		.unwrap();

		let buffer_size = 2 * self.state.transport.frames.get() as usize;
		let mut buf = boxed_slice![0.0; buffer_size];

		let mut updates = Vec::new();

		let mut delay;
		let mut end;

		while {
			delay = self.audio_graph.delay();
			end = len.to_samples(&self.state.transport) + delay;
			self.state.transport.sample < delay
		} {
			let diff = buffer_size.min(delay - self.state.transport.sample);

			self.audio_graph.process(&self.state, &mut buf[..diff]);
			self.audio_graph
				.for_each_node_mut(|node| node.collect_updates(&mut updates));
			updates.clear();

			self.state.transport.sample += diff;
			progress_fn(self.state.transport.sample as f32 / end as f32);
		}

		while {
			delay = self.audio_graph.delay();
			end = len.to_samples(&self.state.transport) + delay;
			self.state.transport.sample < end
		} {
			let diff = buffer_size.min(end - self.state.transport.sample);

			self.audio_graph.process(&self.state, &mut buf[..diff]);
			self.audio_graph
				.for_each_node_mut(|node| node.collect_updates(&mut updates));
			updates.clear();

			for &s in &buf[..diff] {
				writer.write_sample(s).unwrap();
			}

			self.state.transport.sample += diff;
			progress_fn(self.state.transport.sample as f32 / end as f32);
		}

		writer.finalize().unwrap();

		self.state.transport = old;
		self.audio_graph.for_each_node_mut(AudioGraphNode::reset);
	}
}
