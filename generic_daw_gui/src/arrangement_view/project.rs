use super::{ArrangementWrapper, Message, Node, crc};
use crate::{
	arrangement_view::{
		Message as ArrangementMessage, audio_clip::AudioClip, clip::Clip, midi_clip::MidiClip,
		pattern::PatternPair, sample::SamplePair,
	},
	clap_host::ClapHost,
	config::Config,
	daw::Message as DawMessage,
};
use generic_daw_core::{
	ClipPosition, MidiKey, MidiNote, NotePosition, PanMode,
	clap_host::{PluginBundle, PluginDescriptor},
};
use generic_daw_project::{proto, reader::Reader, writer::Writer};
use generic_daw_utils::{NoClone, NoDebug};
use iced::Task;
use smol::{channel::Sender, unblock};
use std::{
	collections::{BTreeMap, HashMap, HashSet},
	fs::File,
	io::{Read as _, Write as _},
	iter::once,
	ops::Deref as _,
	path::Path,
	sync::{Arc, mpsc},
};
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub enum Feedback<T> {
	Use(T),
	Ignore,
	Cancel,
}

impl ArrangementWrapper {
	pub fn save(&self, path: &Path, clap_host: &mut ClapHost) {
		let mut writer = Writer::new(self.rtstate().bpm.into(), self.rtstate().numerator.into());

		let mut samples = HashMap::new();
		for sample in self.samples().values() {
			samples.insert(sample.id, writer.push_sample(&sample.name, sample.crc));
		}

		let mut patterns = HashMap::new();
		for pattern in self.patterns().values() {
			patterns.insert(
				pattern.id,
				writer.push_pattern(pattern.notes.iter().map(|note| proto::Note {
					key: note.key.0.into(),
					velocity: note.velocity,
					position: proto::NotePosition {
						start: note.position.start().into(),
						end: note.position.end().into(),
					},
				})),
			);
		}

		let mut tracks = HashMap::new();
		for track in self.tracks() {
			let node = self.node(track.id);
			tracks.insert(
				track.id,
				writer.push_track(
					track.clips.iter().map(|clip| match clip {
						Clip::Audio(audio) => proto::AudioClip {
							sample: samples[&audio.sample],
							position: proto::ClipPosition {
								position: proto::NotePosition {
									start: audio.position.start().into(),
									end: audio.position.end().into(),
								},
								offset: audio.position.offset().into(),
							},
						}
						.into(),
						Clip::Midi(midi) => proto::MidiClip {
							pattern: patterns[&midi.pattern],
							position: proto::ClipPosition {
								position: proto::NotePosition {
									start: midi.position.start().into(),
									end: midi.position.end().into(),
								},
								offset: midi.position.offset().into(),
							},
						}
						.into(),
					}),
					self.node(track.id)
						.plugins
						.iter()
						.map(|plugin| proto::Plugin {
							id: plugin.descriptor.id.to_bytes_with_nul().to_owned(),
							state: clap_host.get_state(plugin.id),
							mix: plugin.mix,
							enabled: plugin.enabled,
						}),
					node.volume,
					match node.pan {
						PanMode::Balance(pan) => proto::PanModeBalance { pan }.into(),
						PanMode::Stereo(l, r) => proto::PanModeStereo { l, r }.into(),
					},
				),
			);
		}

		let mut channels = HashMap::new();
		for channel in once(self.master()).chain(self.channels()) {
			channels.insert(
				channel.id,
				writer.push_channel(
					self.node(channel.id)
						.plugins
						.iter()
						.map(|plugin| proto::Plugin {
							id: plugin.descriptor.id.to_bytes_with_nul().to_owned(),
							state: clap_host.get_state(plugin.id),
							mix: plugin.mix,
							enabled: plugin.enabled,
						}),
					channel.volume,
					match channel.pan {
						PanMode::Balance(pan) => proto::PanModeBalance { pan }.into(),
						PanMode::Stereo(l, r) => proto::PanModeStereo { l, r }.into(),
					},
				),
			);
		}

		for track in self.tracks() {
			for outgoing in self.outgoing(track.id) {
				writer.connect_track_to_channel(tracks[&track.id], channels[&outgoing]);
			}
		}

		for channel in self.channels() {
			for outgoing in self.outgoing(channel.id) {
				writer.connect_channel_to_channel(channels[&channel.id], channels[&outgoing]);
			}
		}

		File::create(path)
			.unwrap()
			.write_all(&writer.finalize())
			.unwrap();
	}

	pub fn start_load(
		path: Arc<Path>,
		config: Config,
		plugin_bundles: Arc<BTreeMap<PluginDescriptor, PluginBundle>>,
	) -> Task<DawMessage> {
		let (partial_sender, partial_receiver) = oneshot::channel();
		let (progress_sender, progress_receiver) = smol::channel::unbounded();

		Task::batch([
			Task::future(unblock(move || {
				partial_sender
					.send(Self::load(path, &config, &plugin_bundles, &progress_sender))
					.unwrap();
			}))
			.discard(),
			Task::stream(progress_receiver).chain(Task::future(partial_receiver).and_then(
				|tasks| tasks.unwrap_or_else(|| Task::done(DawMessage::OpenedFile(None))),
			)),
		])
	}

	fn load(
		path: Arc<Path>,
		config: &Config,
		plugin_bundles: &BTreeMap<PluginDescriptor, PluginBundle>,
		daw: &Sender<DawMessage>,
	) -> Option<Task<DawMessage>> {
		let config = &config;

		let mut gdp = Vec::new();
		File::open(&path).ok()?.read_to_end(&mut gdp).ok()?;
		let reader = Reader::new(&gdp)?;

		let (mut arrangement, futs) = Self::create(config);

		arrangement.set_bpm(reader.rtstate().bpm as u16);
		arrangement.set_numerator(reader.rtstate().numerator as u8);

		let mut samples = HashMap::new();

		std::thread::scope(|s| {
			let (done, receiver) = mpsc::channel();

			let samples_map = reader
				.iter_samples()
				.map(|(idx, audio)| (&*audio.name, (idx, audio.crc)))
				.fold(HashMap::<_, Vec<_>>::new(), |mut acc, (k, v)| {
					acc.entry(k).or_default().push(v);
					acc
				});

			let mut current_progress = 0.0;
			let progress_per_audio = 1.0 / (reader.iter_samples().count() as f32);

			let mut seen = HashSet::new();

			let mut paths = config
				.sample_paths
				.iter()
				.flat_map(WalkDir::new)
				.flatten()
				.filter(|dir_entry| dir_entry.file_type().is_file())
				.filter_map(|dir_entry| {
					dir_entry
						.file_name()
						.to_str()
						.filter(|name| samples_map.contains_key(name))
						.filter(|_| seen.insert(dir_entry.path().to_owned()))
						.and_then(|name| {
							File::open(dir_entry.path())
								.ok()
								.map(|file| (name, crc(file)))
						})
						.and_then(|(name, crc)| {
							samples_map[name].iter().find_map(|&(index, c)| {
								(c == crc).then(|| (index, Arc::<Path>::from(dir_entry.path())))
							})
						})
				})
				.collect::<HashMap<_, _>>();

			drop(seen);

			for (idx, sample) in reader.iter_samples() {
				let sample_rate = arrangement.rtstate().sample_rate;
				let done = done.clone();

				let mut path = paths.remove(&idx);
				let mut sample_name = sample.name.clone();
				s.spawn(move || {
					loop {
						if let Some(path) = &path
							&& let Some(sample) = SamplePair::new(path, sample_rate)
						{
							done.send((idx, Feedback::Use(sample))).unwrap();
							return;
						}

						if let Some(path) = &path
							&& let Some(name) = path.file_name()
							&& let Some(name) = name.to_str()
						{
							sample_name = name.to_owned();
						}

						let (sender, receiver) = oneshot::channel();

						daw.try_send(DawMessage::CantLoadSample(
							sample_name.deref().into(),
							NoClone(sender),
						))
						.unwrap();

						path = match receiver.recv() {
							Ok(Feedback::Use(p)) => Some(p),
							Ok(Feedback::Ignore) => {
								_ = done.send((idx, Feedback::Ignore));
								return;
							}
							Ok(Feedback::Cancel) | Err(_) => {
								_ = done.send((idx, Feedback::Cancel));
								return;
							}
						};
					}
				});
			}

			drop(paths);
			drop(done);

			while let Ok((idx, sample)) = receiver.recv() {
				match sample {
					Feedback::Use(sample) => {
						let id = sample.gui.id;
						arrangement.add_sample(sample);
						samples.insert(idx, Feedback::Use(id));
					}
					Feedback::Ignore => _ = samples.insert(idx, Feedback::Ignore),
					Feedback::Cancel => {
						daw.try_send(DawMessage::OpenedFile(None)).unwrap();
						return None;
					}
				}
				current_progress += progress_per_audio;
				daw.try_send(DawMessage::Progress(current_progress))
					.unwrap();
			}

			Some(())
		})?;

		let mut patterns = HashMap::new();
		for (idx, notes) in reader.iter_patterns() {
			let pattern = notes
				.notes
				.iter()
				.map(|note| MidiNote {
					key: MidiKey(note.key as u8),
					velocity: note.velocity,
					position: NotePosition::new(
						note.position.start.into(),
						note.position.end.into(),
					),
				})
				.collect();

			let pattern = PatternPair::new(pattern);
			let id = pattern.gui.id;
			arrangement.add_pattern(pattern);
			patterns.insert(idx, id);
		}

		let mut messages = Vec::new();

		let mut load_channel = |node: &Node, channel: &proto::Channel| {
			if channel.volume != 1.0 {
				messages.push(Message::ChannelVolumeChanged(node.id, channel.volume));
			}

			if channel.pan.pan_mode? != proto::PanMode::Balance(proto::PanModeBalance { pan: 0.0 })
			{
				messages.push(Message::ChannelPanChanged(
					node.id,
					match channel.pan.pan_mode? {
						proto::PanMode::Balance(proto::PanModeBalance { pan }) => {
							PanMode::Balance(pan)
						}
						proto::PanMode::Stereo(proto::PanModeStereo { l, r }) => {
							PanMode::Stereo(l, r)
						}
					},
				));
			}

			for (i, plugin) in channel.plugins.iter().enumerate() {
				let id = plugin.id();
				messages.push(Message::PluginLoad(
					node.id,
					plugin_bundles.keys().find(|d| *d.id == id)?.clone(),
					false,
				));

				if let Some(state) = plugin.state.as_deref() {
					messages.push(Message::PluginSetState(
						node.id,
						i,
						NoDebug(Box::from(state)),
					));
				}

				if plugin.mix != 1.0 {
					messages.push(Message::PluginMixChanged(node.id, i, plugin.mix));
				}

				if !plugin.enabled {
					messages.push(Message::PluginToggleEnabled(node.id, i));
				}
			}

			Some(())
		};

		let mut tracks = HashMap::new();
		for (idx, clips, channel) in reader.iter_tracks() {
			let track = arrangement.add_track();
			let id = arrangement.tracks()[track].id;
			tracks.insert(idx, id);
			load_channel(arrangement.node(id), channel)?;

			for clip in clips {
				arrangement.add_clip(
					track,
					match clip {
						proto::Clip::Audio(audio) => {
							let Feedback::Use(sample) = samples.get(&audio.sample)? else {
								continue;
							};
							let mut clip = AudioClip::new(*sample);
							clip.position = ClipPosition::new(
								NotePosition::new(
									audio.position.position.start.into(),
									audio.position.position.end.into(),
								),
								audio.position.offset.into(),
							);
							Clip::Audio(clip)
						}
						proto::Clip::Midi(midi) => {
							let mut clip = MidiClip::new(*patterns.get(&midi.pattern)?);
							clip.position = ClipPosition::new(
								NotePosition::new(
									midi.position.position.start.into(),
									midi.position.position.end.into(),
								),
								midi.position.offset.into(),
							);
							Clip::Midi(clip)
						}
					},
				);
			}
		}

		let mut channels = HashMap::new();
		let mut iter_channels = reader.iter_channels();

		let node = arrangement.master();
		let (idx, channel) = iter_channels.next()?;
		load_channel(node, channel)?;
		channels.insert(idx, node.id);

		for (idx, channel) in iter_channels {
			let id = arrangement.add_channel();
			channels.insert(idx, id);
			load_channel(arrangement.node(id), channel)?;
		}

		for (from, to) in reader.iter_track_to_channel() {
			messages.push(Message::ConnectRequest(
				*tracks.get(&from)?,
				*channels.get(&to)?,
			));
		}

		drop(tracks);

		for (from, to) in reader.iter_channel_to_channel() {
			messages.push(Message::ConnectRequest(
				*channels.get(&from)?,
				*channels.get(&to)?,
			));
		}

		drop(channels);

		Some(
			Task::done(DawMessage::Arrangement(ArrangementMessage::SetArrangement(
				NoClone(Box::new(arrangement)),
			)))
			.chain(Task::batch([
				futs.map(Message::Batch).map(DawMessage::Arrangement),
				messages
					.into_iter()
					.map(Task::done)
					.fold(Task::none(), Task::chain)
					.map(DawMessage::Arrangement)
					.chain(Task::done(DawMessage::OpenedFile(Some(path)))),
			])),
		)
	}
}
