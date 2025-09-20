use super::{ArrangementView, ArrangementWrapper, LoadStatus, Message, Node, crc};
use crate::{config::Config, daw::Message as DawMessage};
use arc_swap::ArcSwap;
use generic_daw_core::{
	AudioClip, Clip, MidiClip, MidiKey, MidiNote, Mixer, NodeImpl as _, Sample, Track,
	clap_host::{PluginBundle, PluginDescriptor},
};
use generic_daw_project::{proto, reader::Reader, writer::Writer};
use generic_daw_utils::NoClone;
use iced::{Task, widget::combo_box};
use log::info;
use smol::unblock;
use std::{
	collections::{BTreeMap, HashMap},
	fs::File,
	io::{Read as _, Write as _},
	iter::once,
	path::Path,
	sync::{Arc, Weak, mpsc},
};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct PartialArrangementView {
	arrangement: ArrangementWrapper,
	audios: Vec<(Arc<Path>, LoadStatus)>,
	midis: Vec<Weak<ArcSwap<Vec<MidiNote>>>>,
}

impl ArrangementView {
	pub fn apply_partial(
		&mut self,
		partial: PartialArrangementView,
		plugin_bundles: &BTreeMap<PluginDescriptor, PluginBundle>,
	) {
		self.arrangement = partial.arrangement;
		self.audios.extend(partial.audios);
		self.midis.extend(partial.midis);
		self.plugins = combo_box::State::new(plugin_bundles.keys().cloned().collect());
		self.recording = None;
		self.soloed_track = None;
		self.selected_channel = None;
	}

	pub fn save(&mut self, path: &Path) {
		let mut writer = Writer::new(
			self.arrangement.rtstate().bpm.into(),
			self.arrangement.rtstate().numerator.into(),
		);

		let mut audios = HashMap::new();
		for entry in &self.audios {
			let (crc, audio) = match entry.1 {
				LoadStatus::Loaded(crc, audio) => {
					let Some(audio) = audio.upgrade() else {
						continue;
					};
					(crc, audio)
				}
				LoadStatus::Loading(..) => continue,
			};
			audios.insert(audio.path.clone(), writer.push_audio(&audio.name, *crc));
		}

		let mut midis = HashMap::new();
		for entry in &self.midis {
			let Some(pattern) = entry.upgrade() else {
				continue;
			};

			midis.insert(
				Arc::as_ptr(&pattern).addr(),
				writer.push_midi(pattern.load().iter().map(|note| proto::Note {
					key: note.key.0.into(),
					velocity: note.velocity,
					start: note.start.into(),
					end: note.end.into(),
				})),
			);
		}

		let mut tracks = HashMap::new();
		for track in self.arrangement.tracks() {
			let node = self.arrangement.node(track.id);
			tracks.insert(
				track.id,
				writer.push_track(
					track.clips.iter().map(|clip| match clip {
						Clip::Audio(audio) => proto::AudioClip {
							audio: audios[&audio.sample.path],
							position: proto::ClipPosition {
								start: audio.position.start().into(),
								end: audio.position.end().into(),
								offset: audio.position.offset().into(),
							},
						}
						.into(),
						Clip::Midi(midi) => proto::MidiClip {
							midi: midis[&Arc::as_ptr(&midi.pattern).addr()],
							position: proto::ClipPosition {
								start: midi.position.start().into(),
								end: midi.position.end().into(),
								offset: midi.position.offset().into(),
							},
						}
						.into(),
					}),
					self.arrangement
						.node(track.id)
						.plugins
						.iter()
						.map(|plugin| proto::Plugin {
							id: plugin.descriptor.id.to_bytes_with_nul().to_owned(),
							state: self.clap_host.get_state(plugin.id),
							mix: plugin.mix,
							enabled: plugin.enabled,
						}),
					node.volume,
					node.pan,
				),
			);
		}

		let mut channels = HashMap::new();
		for channel in once(self.arrangement.master()).chain(self.arrangement.channels()) {
			channels.insert(
				channel.id,
				writer.push_channel(
					self.arrangement
						.node(channel.id)
						.plugins
						.iter()
						.map(|plugin| proto::Plugin {
							id: plugin.descriptor.id.to_bytes_with_nul().to_owned(),
							state: self.clap_host.get_state(plugin.id),
							mix: plugin.mix,
							enabled: plugin.enabled,
						}),
					channel.volume,
					channel.pan,
				),
			);
		}

		for track in self.arrangement.tracks() {
			for outgoing in self.arrangement.outgoing(track.id) {
				writer.connect_track_to_channel(tracks[&track.id], channels[&outgoing]);
			}
		}

		for channel in self.arrangement.channels() {
			for outgoing in self.arrangement.outgoing(channel.id) {
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
					.send(Self::load(path, &config, &plugin_bundles, |f| {
						progress_sender.try_send(f).unwrap();
					}))
					.unwrap();
			}))
			.discard(),
			Task::stream(progress_receiver)
				.map(DawMessage::Progress)
				.chain(Task::future(partial_receiver).and_then(|tasks| {
					tasks.unwrap_or_else(|| Task::done(DawMessage::OpenedFile(None)))
				})),
		])
	}

	fn load(
		path: Arc<Path>,
		config: &Config,
		plugin_bundles: &BTreeMap<PluginDescriptor, PluginBundle>,
		mut progress_fn: impl FnMut(f32),
	) -> Option<Task<DawMessage>> {
		info!("loading project {}", path.display());

		let config = &config;

		let mut gdp = Vec::new();
		File::open(&path).ok()?.read_to_end(&mut gdp).ok()?;
		let reader = Reader::new(&gdp)?;

		let (mut arrangement, futs) = ArrangementWrapper::create(config);

		arrangement.set_bpm(reader.rtstate().bpm as u16);
		arrangement.set_numerator(reader.rtstate().numerator as u8);

		let mut audios = HashMap::new();

		let (sender, receiver) = mpsc::channel();
		let audios_map = reader
			.iter_audios()
			.map(|(idx, audio)| (&*audio.name, (idx, audio.crc)))
			.collect::<HashMap<_, _>>();

		std::thread::scope(|s| {
			config
				.sample_paths
				.iter()
				.flat_map(WalkDir::new)
				.flatten()
				.filter(|dir_entry| dir_entry.file_type().is_file())
				.filter_map(|dir_entry| {
					dir_entry
						.file_name()
						.to_str()
						.filter(|name| audios_map.contains_key(name))
						.map(|name| (name.to_owned(), dir_entry.path().to_owned()))
				})
				.filter(|(name, path)| {
					File::open(path).is_ok_and(|file| crc(file) == audios_map[&**name].1)
				})
				.for_each(|(name, path)| {
					let audios_map = &audios_map;
					let sender = sender.clone();
					let sample_rate = arrangement.rtstate().sample_rate;
					s.spawn(move || {
						sender
							.send((
								audios_map[&*name].0,
								Sample::create(path.into(), sample_rate)
									.map(|sample| (audios_map[&*name].1, sample)),
							))
							.unwrap();
					});
				});

			drop(sender);

			let mut current_progress = 0.0;
			let progress_per_audio = 1.0 / (reader.iter_audios().count() as f32);
			while let Ok((idx, audio)) = receiver.recv() {
				audios.insert(idx, audio?);
				current_progress += progress_per_audio;
				progress_fn(current_progress);
			}

			Some(())
		})?;

		let mut midis = HashMap::new();
		for (idx, notes) in reader.iter_midis() {
			let pattern = notes
				.notes
				.iter()
				.map(|note| MidiNote {
					key: MidiKey(note.key as u8),
					velocity: note.velocity,
					start: note.start.into(),
					end: note.end.into(),
				})
				.collect();

			midis.insert(idx, Arc::new(ArcSwap::new(Arc::new(pattern))));
		}

		let mut messages = Vec::new();

		let mut load_channel = |node: &Node, channel: &proto::Channel| {
			if channel.volume != 1.0 {
				messages.push(Message::ChannelVolumeChanged(node.id, channel.volume));
			}

			if channel.pan != 0.0 {
				messages.push(Message::ChannelPanChanged(node.id, channel.pan));
			}

			for (i, plugin) in channel.plugins.iter().enumerate() {
				let id = plugin.id();
				messages.push(Message::PluginLoad(
					node.id,
					plugin_bundles.keys().find(|d| *d.id == id)?.clone(),
					false,
				));

				if let Some(state) = plugin.state.clone() {
					messages.push(Message::PluginSetState(
						node.id,
						i,
						state.into_boxed_slice().into(),
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
			let mut track = Track::default();

			for clip in clips {
				track.clips.push(match clip {
					proto::Clip::Audio(audio) => {
						let clip = AudioClip::create(
							audios.get(&audio.audio)?.1.clone(),
							arrangement.rtstate(),
						);
						clip.position.trim_start_to(audio.position.offset.into());
						clip.position.move_to(audio.position.start.into());
						clip.position.trim_end_to(audio.position.end.into());
						Clip::Audio(clip)
					}
					proto::Clip::Midi(midi) => {
						let clip = MidiClip::create(midis.get(&midi.midi)?.clone());
						clip.position.trim_start_to(midi.position.offset.into());
						clip.position.move_to(midi.position.start.into());
						clip.position.trim_end_to(midi.position.end.into());
						Clip::Midi(clip)
					}
				});
			}

			let id = track.id();
			tracks.insert(idx, id);
			arrangement.push_track(track);
			load_channel(arrangement.node(id), channel)?;
		}

		let mut channels = HashMap::new();
		let mut iter_channels = reader.iter_channels();

		let node = arrangement.master();
		let (idx, channel) = iter_channels.next()?;
		load_channel(node, channel)?;
		channels.insert(idx, node.id);

		for (idx, channel) in iter_channels {
			let mixer_node = Mixer::default();
			let id = mixer_node.id();
			channels.insert(idx, id);
			arrangement.push_channel(mixer_node);
			load_channel(arrangement.node(id), channel)?;
		}

		for (from, to) in reader.iter_track_to_channel() {
			messages.push(Message::ConnectRequest((
				*tracks.get(&from)?,
				*channels.get(&to)?,
			)));
		}

		for (from, to) in reader.iter_channel_to_channel() {
			messages.push(Message::ConnectRequest((
				*channels.get(&from)?,
				*channels.get(&to)?,
			)));
		}

		info!("loaded project {}", path.display());

		let partial = PartialArrangementView {
			arrangement,
			audios: audios
				.values()
				.map(|(crc, audio)| {
					(
						audio.path.clone(),
						LoadStatus::Loaded(*crc, Arc::downgrade(audio)),
					)
				})
				.collect(),
			midis: midis.values().map(Arc::downgrade).collect(),
		};

		Some(
			Task::done(DawMessage::ApplyPartial(NoClone(Box::new(partial)))).chain(Task::batch([
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

	pub fn unload(
		&mut self,
		config: &Config,
		plugin_bundles: &BTreeMap<PluginDescriptor, PluginBundle>,
	) -> Task<Message> {
		let (arrangement, futs) = ArrangementWrapper::create(config);

		self.apply_partial(
			PartialArrangementView {
				arrangement,
				audios: Vec::new(),
				midis: Vec::new(),
			},
			plugin_bundles,
		);

		futs.map(Message::Batch)
	}
}
