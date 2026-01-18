#![warn(clippy::iter_over_hash_type)]

use crate::{
	arrangement_view::{
		self, Arrangement, Node, audio_clip::AudioClip, clip::Clip, crc, midi_clip::MidiClip,
		midi_pattern::MidiPatternPair, sample::SamplePair,
	},
	clap_host::ClapHost,
	config::Config,
	daw,
};
use generic_daw_core::{
	MidiKey, MidiNote, MusicalTime, OffsetPosition, PanMode, Position,
	clap_host::{PluginBundle, PluginDescriptor},
};
use generic_daw_project::{proto, reader::Reader, writer::Writer};
use iced::Task;
use smol::{channel::Sender, unblock};
use std::{
	collections::{HashMap, HashSet},
	fs::File,
	io::{Read as _, Write as _},
	iter::once,
	num::NonZero,
	ops::Deref as _,
	path::Path,
	sync::{Arc, mpsc},
};
use utils::NoDebug;
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub enum Feedback<T> {
	Use(T),
	Ignore,
	Cancel,
}

impl Arrangement {
	pub fn save(&self, path: &Path, clap_host: &mut ClapHost) {
		let mut writer = Writer::new(proto::Transport {
			bpm: self.transport().bpm.get().into(),
			numerator: self.transport().numerator.get().into(),
			loop_marker: self
				.transport()
				.loop_marker
				.map(|loop_marker| proto::Position {
					start: loop_marker.start().into_raw(),
					end: loop_marker.end().into_raw(),
				}),
		});

		let mut samples = HashMap::new();
		for sample in self.samples().values() {
			samples.insert(sample.id, writer.push_sample(&sample.name, sample.crc));
		}

		let mut midi_patterns = HashMap::new();
		for pattern in self.midi_patterns().values() {
			midi_patterns.insert(
				pattern.id,
				writer.push_pattern(
					&pattern.name,
					pattern.notes.iter().map(|note| proto::Note {
						key: note.key.0.into(),
						velocity: note.velocity,
						position: proto::Position {
							start: note.position.start().into_raw(),
							end: note.position.end().into_raw(),
						},
					}),
				),
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
							position: proto::OffsetPosition {
								position: proto::Position {
									start: audio.position.start().into_raw(),
									end: audio.position.end().into_raw(),
								},
								offset: audio.position.offset().into_raw(),
							},
						}
						.into(),
						Clip::Midi(midi) => proto::MidiClip {
							pattern: midi_patterns[&midi.pattern],
							position: proto::OffsetPosition {
								position: proto::Position {
									start: midi.position.start().into_raw(),
									end: midi.position.end().into_raw(),
								},
								offset: midi.position.offset().into_raw(),
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
					node.enabled,
					node.bypassed,
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
					channel.enabled,
					channel.bypassed,
				),
			);
		}

		for track in self.tracks() {
			for outgoing in self.outgoing(track.id) {
				writer.connect_track_to_channel(tracks[&track.id], channels[outgoing]);
			}
		}

		for channel in self.channels() {
			for outgoing in self.outgoing(channel.id) {
				writer.connect_channel_to_channel(channels[&channel.id], channels[outgoing]);
			}
		}

		File::create(path)
			.unwrap()
			.write_all(&writer.finalize())
			.unwrap();
	}

	pub fn empty(config: Config) -> Task<daw::Message> {
		let (wrapper, task) = Self::create(&config);
		Task::done(daw::Message::MergeConfig(config.into(), false))
			.chain(Task::done(daw::Message::Arrangement(
				arrangement_view::Message::SetArrangement(Box::new(wrapper).into()),
			)))
			.chain(
				task.map(arrangement_view::Message::Batch)
					.map(daw::Message::Arrangement),
			)
	}

	pub fn start_load(
		path: Arc<Path>,
		config: Config,
		plugin_bundles: Arc<HashMap<PluginDescriptor, NoDebug<PluginBundle>>>,
	) -> Task<daw::Message> {
		let (partial_sender, partial_receiver) = oneshot::channel();
		let (progress_sender, progress_receiver) = smol::channel::unbounded();

		Task::batch([
			Task::future(unblock(move || {
				partial_sender
					.send(Self::do_load(
						path,
						config,
						&plugin_bundles,
						&progress_sender,
					))
					.unwrap();
			}))
			.discard(),
			Task::stream(progress_receiver).chain(
				Task::perform(partial_receiver, Result::ok).and_then(|tasks| {
					tasks.unwrap_or_else(|| Task::done(daw::Message::OpenedFile(None)))
				}),
			),
		])
	}

	fn do_load(
		path: Arc<Path>,
		config: Config,
		plugin_bundles: &HashMap<PluginDescriptor, NoDebug<PluginBundle>>,
		daw: &Sender<daw::Message>,
	) -> Option<Task<daw::Message>> {
		let mut gdp = Vec::new();
		File::open(&path).ok()?.read_to_end(&mut gdp).ok()?;
		let reader = Reader::new(&gdp)?;

		let (mut arrangement, task) = Self::create(&config);

		let proto::Transport {
			bpm,
			numerator,
			loop_marker,
		} = reader.transport();

		arrangement.set_bpm(NonZero::new(bpm as u16)?);
		arrangement.set_numerator(NonZero::new(numerator as u8)?);
		arrangement.set_loop_marker(loop_marker.map(|loop_marker| {
			Position::new(
				MusicalTime::from_raw(loop_marker.start),
				MusicalTime::from_raw(loop_marker.end),
			)
		}));

		let mut samples = HashMap::new();

		rayon_core::in_place_scope(|s| {
			let (done, receiver) = mpsc::channel();

			let samples_map = reader
				.iter_samples()
				.map(|(idx, audio)| (&*audio.name, (idx, audio.crc)))
				.fold(HashMap::<_, Vec<_>>::new(), |mut acc, (k, v)| {
					acc.entry(k).or_default().push(v);
					acc
				});

			let mut current_progress = 0.0;
			let progress_per_audio = (reader.iter_samples().count() as f32).recip();

			let mut seen = HashSet::new();
			let mut paths = config
				.sample_paths
				.iter()
				.flat_map(|path| WalkDir::new(path).follow_links(true))
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
								(c == crc).then(|| (index, dir_entry.path().into()))
							})
						})
				})
				.collect::<HashMap<_, _>>();

			drop(seen);
			drop(samples_map);

			for (idx, sample) in reader.iter_samples() {
				let sample_rate = arrangement.transport().sample_rate;
				let done = done.clone();

				let mut path: Option<Arc<_>> = paths.remove(&idx);
				let mut sample_name = sample.name.clone();
				s.spawn(move |_| {
					loop {
						if let Some(path) = path.clone()
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

						daw.try_send(daw::Message::CantLoadSample(
							sample_name.deref().into(),
							sender.into(),
						))
						.unwrap();

						path = match receiver.recv() {
							Ok(Feedback::Use(p)) => Some(p),
							Ok(Feedback::Ignore) => {
								_ = done.send((idx, Feedback::Ignore));
								return;
							}
							Ok(Feedback::Cancel) | Err(..) => {
								_ = done.send((idx, Feedback::Cancel));
								return;
							}
						};
					}
				});
			}

			drop(paths);
			drop(done);

			for (idx, sample) in receiver {
				match sample {
					Feedback::Use(sample) => {
						let id = sample.gui.id;
						arrangement.add_sample(sample);
						samples.insert(idx, Feedback::Use(id));
					}
					Feedback::Ignore => _ = samples.insert(idx, Feedback::Ignore),
					Feedback::Cancel => {
						daw.try_send(daw::Message::OpenedFile(None)).unwrap();
						return None;
					}
				}
				current_progress += progress_per_audio;
				daw.try_send(daw::Message::Progress(current_progress))
					.unwrap();
			}

			Some(())
		})?;

		let mut midi_patterns = HashMap::new();
		for (idx, pattern) in reader.iter_midi_patterns() {
			let notes = pattern
				.notes
				.iter()
				.map(|note| MidiNote {
					key: MidiKey(note.key as u8),
					velocity: note.velocity,
					position: Position::new(
						MusicalTime::from_raw(note.position.start),
						MusicalTime::from_raw(note.position.end),
					),
				})
				.collect();

			let pattern = MidiPatternPair::from_notes(notes, &pattern.name);
			let id = pattern.gui.id;
			arrangement.add_midi_pattern(pattern);
			midi_patterns.insert(idx, id);
		}

		let mut messages = Vec::new();

		let mut load_channel = |node: &Node, channel: &proto::Channel| {
			if channel.volume != 1.0 {
				messages.push(arrangement_view::Message::ChannelVolumeChanged(
					node.id,
					channel.volume,
				));
			}

			if channel.pan.pan_mode? != proto::PanMode::Balance(proto::PanModeBalance { pan: 0.0 })
			{
				messages.push(arrangement_view::Message::ChannelPanChanged(
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

			if !channel.enabled {
				messages.push(arrangement_view::Message::ChannelToggleEnabled(node.id));
			}

			if channel.bypassed {
				messages.push(arrangement_view::Message::ChannelToggleBypassed(node.id));
			}

			for (i, plugin) in channel.plugins.iter().enumerate() {
				let id = plugin.id();
				messages.push(arrangement_view::Message::PluginLoad(
					node.id,
					plugin_bundles.keys().find(|d| *d.id == id)?.clone(),
					false,
				));

				if let Some(state) = plugin.state.as_deref() {
					messages.push(arrangement_view::Message::PluginSetState(
						node.id,
						i,
						NoDebug(Box::from(state)),
					));
				}

				if plugin.mix != 1.0 {
					messages.push(arrangement_view::Message::PluginMixChanged(
						node.id, i, plugin.mix,
					));
				}

				if !plugin.enabled {
					messages.push(arrangement_view::Message::PluginToggleEnabled(node.id, i));
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
							Clip::Audio(AudioClip {
								sample: *sample,
								position: OffsetPosition::new(
									Position::new(
										MusicalTime::from_raw(audio.position.position.start),
										MusicalTime::from_raw(audio.position.position.end),
									),
									MusicalTime::from_raw(audio.position.offset),
								),
							})
						}
						proto::Clip::Midi(midi) => Clip::Midi(MidiClip {
							pattern: *midi_patterns.get(&midi.pattern)?,
							position: OffsetPosition::new(
								Position::new(
									MusicalTime::from_raw(midi.position.position.start),
									MusicalTime::from_raw(midi.position.position.end),
								),
								MusicalTime::from_raw(midi.position.offset),
							),
						}),
					},
				);
			}
		}

		drop(samples);
		drop(midi_patterns);

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
			messages.push(arrangement_view::Message::Connect(
				*tracks.get(&from)?,
				*channels.get(&to)?,
			));
		}

		drop(tracks);

		for (from, to) in reader.iter_channel_to_channel() {
			messages.push(arrangement_view::Message::Connect(
				*channels.get(&from)?,
				*channels.get(&to)?,
			));
		}

		drop(channels);
		drop(reader);

		Some(
			Task::done(daw::Message::MergeConfig(config.into(), false))
				.chain(Task::done(daw::Message::Arrangement(
					arrangement_view::Message::SetArrangement(Box::new(arrangement).into()),
				)))
				.chain(Task::batch([
					task.map(arrangement_view::Message::Batch)
						.map(daw::Message::Arrangement),
					messages
						.into_iter()
						.map(Task::done)
						.fold(Task::none(), Task::chain)
						.map(daw::Message::Arrangement)
						.chain(Task::done(daw::Message::OpenedFile(Some(path)))),
				])),
		)
	}
}
