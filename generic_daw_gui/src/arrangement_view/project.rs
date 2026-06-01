#![warn(clippy::iter_over_hash_type)]

use crate::{
	arrangement_view::{
		self, Arrangement, Node, crc, midi_pattern::MidiPatternPair, sample::SamplePair,
	},
	clap_host::ClapHost,
	config::Config,
	daw::{self, Project},
};
use generic_daw_core::{
	AudioClip, Clip, ClipId, MidiClip, MidiKey, MidiNote, MidiNoteId, PanMode, Point, Transition,
	clap_host::{PluginDescriptor, StateContextType},
	time::{BeatRange, BeatSpan, BeatTime, OffsetBeatRange, OffsetBeatSpan, SecondsTime},
};
use generic_daw_project::{Reader, Writer, proto};
use iced::Task;
use smol::{channel::Sender, unblock};
use std::{
	collections::{HashMap, HashSet},
	fs::File,
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
	pub fn save(&self, clap_host: &mut ClapHost, view: proto::ViewState) -> Vec<u8> {
		let mut writer = Writer::new(proto::Transport {
			bpm: self.transport().bpm.get().into(),
			numerator: self.transport().numerator.get().into(),
			loop_range: self
				.transport()
				.loop_range
				.map(|loop_range| proto::BeatRange {
					start: loop_range.start().to_bits(),
					end: loop_range.end().to_bits(),
				}),
		});

		let mut samples = HashMap::new();
		for sample in self.samples().values() {
			samples.insert(
				sample.id,
				writer.push_sample(&sample.name, sample.crc, sample.len),
			);
		}

		let mut midi_patterns = HashMap::new();
		for pattern in self.midi_patterns().values() {
			midi_patterns.insert(
				pattern.id,
				writer.push_midi_pattern(
					&pattern.name,
					pattern.notes.iter().map(|note| proto::Note {
						key: note.key.0.into(),
						velocity: note.velocity,
						position: proto::BeatRange {
							start: note.position.start().to_bits(),
							end: note.position.end().to_bits(),
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
						Clip::Audio(clip) => proto::AudioClip {
							sample: samples[&clip.sample],
							position_compat: None,
							stretch_compat: None,
							position: proto::OffsetBeatSpan {
								position: proto::BeatSpan {
									start: clip.position.start().to_bits(),
									len: clip.position.len().to_bits(),
								},
								offset: clip.position.offset().to_bits(),
							},
							volume: clip.volume,
							fade_start: proto::Transition {
								len: clip.fade_start.len.to_bits(),
								p: proto::Point {
									x: clip.fade_start.p.x,
									y: clip.fade_start.p.y,
								},
								symmetric: clip.fade_start.symmetric,
							},
							fade_end: proto::Transition {
								len: clip.fade_end.len.to_bits(),
								p: proto::Point {
									x: clip.fade_end.p.x,
									y: clip.fade_end.p.y,
								},
								symmetric: clip.fade_end.symmetric,
							},
							stretch: clip.stretch,
						}
						.into(),
						Clip::Midi(clip) => proto::MidiClip {
							pattern: midi_patterns[&clip.pattern],
							position: proto::OffsetBeatRange {
								position: proto::BeatRange {
									start: clip.position.start().to_bits(),
									end: clip.position.end().to_bits(),
								},
								offset: clip.position.offset().to_bits(),
							},
						}
						.into(),
					}),
					self.node(track.id)
						.plugins
						.iter()
						.map(|plugin| proto::Plugin {
							id: plugin.descriptor.id.to_bytes_with_nul().to_owned(),
							state: clap_host
								.get_state(plugin.id, StateContextType::ForProject)
								.map(Vec::from),
							mix: plugin.mix,
							active: plugin.active,
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
		for channel in
			once(self.master()).chain(self.channels().iter().map(|channel| self.node(channel.id)))
		{
			channels.insert(
				channel.id,
				writer.push_channel(
					self.node(channel.id)
						.plugins
						.iter()
						.map(|plugin| proto::Plugin {
							id: plugin.descriptor.id.to_bytes_with_nul().to_owned(),
							state: clap_host
								.get_state(plugin.id, StateContextType::ForProject)
								.map(Vec::from),
							mix: plugin.mix,
							active: plugin.active,
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
			for (outgoing, &mix) in self.outgoing(track.id) {
				writer.connect_track_to_channel(tracks[&track.id], channels[outgoing], mix);
			}
		}

		for channel in self.channels() {
			for (outgoing, &mix) in self.outgoing(channel.id) {
				writer.connect_channel_to_channel(channels[&channel.id], channels[outgoing], mix);
			}
		}

		writer.set_view(view);

		writer.finalize()
	}

	pub fn empty() -> Task<daw::Message> {
		let config = Config::read();

		let project = Project::unique();
		let (arrangement, task) = Self::create(&config);

		Task::done(daw::Message::MergeConfig(config.into(), false))
			.chain(Task::done(daw::Message::ProjectLoaded(
				project,
				Box::new(arrangement).into(),
				None,
			)))
			.chain(
				task.map(arrangement_view::Message::Batch)
					.map(move |message| daw::Message::Arrangement(project, message)),
			)
	}

	pub fn start_load(
		path: Arc<Path>,
		plugin_bundles: Vec<PluginDescriptor>,
	) -> Task<daw::Message> {
		let (partial_sender, partial_receiver) = oneshot::channel();
		let (progress_sender, progress_receiver) = smol::channel::unbounded();

		Task::batch([
			Task::future(unblock(move || {
				partial_sender
					.send(Self::do_load(path, &plugin_bundles, &progress_sender))
					.unwrap();
			}))
			.discard(),
			Task::stream(progress_receiver).chain(
				Task::perform(partial_receiver.into_future(), Result::ok).and_then(|tasks| {
					tasks.unwrap_or_else(|| Task::done(daw::Message::OpenedFile(None)))
				}),
			),
		])
	}

	fn do_load(
		path: Arc<Path>,
		plugin_bundles: &[PluginDescriptor],
		daw: &Sender<daw::Message>,
	) -> Option<Task<daw::Message>> {
		let config = Config::read();
		let (mut arrangement, task) = Self::create(&config);

		let gdp = std::fs::read(&path).ok()?;
		let reader = Reader::new(&gdp)?;

		let proto::Transport {
			bpm,
			numerator,
			loop_range,
		} = reader.transport();

		arrangement.set_bpm(NonZero::new(bpm as u16)?);
		arrangement.set_numerator(NonZero::new(numerator as u8)?);
		arrangement.set_loop_range(loop_range.map(|loop_range| {
			BeatRange::new(
				BeatTime::from_bits(loop_range.start),
				BeatTime::from_bits(loop_range.end),
			)
		}));

		let mut samples = HashMap::<_, HashMap<_, HashMap<_, _>>>::new();
		for (index, sample) in reader.iter_samples() {
			samples
				.entry(&*sample.name)
				.or_default()
				.entry(sample.len)
				.or_default()
				.insert(sample.crc, index);
		}

		let mut current_progress = 0.0;
		let progress_per_audio = (reader.iter_samples().count() as f32).recip();

		let mut seen = HashSet::new();
		let mut paths = config
			.sample_paths
			.iter()
			.flat_map(|path| WalkDir::new(path).follow_links(true))
			.flatten()
			.filter(|dir_entry| dir_entry.file_type().is_file())
			.filter_map(|dir_entry| dir_entry.path().canonicalize().ok())
			.filter_map(|path| {
				path.file_name()
					.and_then(|name| name.to_str())
					.filter(|name| samples.contains_key(name))
					.and_then(|name| std::fs::metadata(&path).ok().map(|meta| (name, meta.len())))
					.filter(|(name, len)| samples[name].contains_key(len))
					.filter(|_| seen.insert(path.clone()))
					.and_then(|(name, len)| {
						daw.try_send(daw::Message::SetStatus(path.to_string_lossy().into()))
							.unwrap();
						let crc = File::open(&path).ok().map(crc);
						daw.try_send(daw::Message::ClearStatus).unwrap();
						crc.map(|crc| (name, crc, len))
					})
					.and_then(|(name, crc, len)| {
						samples[name][&len]
							.get(&crc)
							.map(|&index| (index, path.as_path().into()))
					})
			})
			.collect::<HashMap<_, _>>();

		drop(seen);
		drop(samples);

		let mut samples = HashMap::new();

		let (done, receiver) = mpsc::channel();

		for (index, sample) in reader.iter_samples() {
			let done = done.clone();
			let path = paths.remove(&index);
			let sample = sample.clone();

			std::thread::spawn(move || {
				if let Some(path) = path
					&& let Some(sample) = SamplePair::with_crc_and_len(path, sample.crc, sample.len)
				{
					done.send((index, Feedback::Use(Ok(sample)))).unwrap();
					return;
				}

				loop {
					let (sender, receiver) = oneshot::channel();

					done.send((
						index,
						Feedback::Use(Err(daw::Message::CantFindSample(
							sample.name.deref().into(),
							sender.into(),
						))),
					))
					.unwrap();

					let path = match receiver.recv() {
						Ok(Feedback::Use(path)) => path,
						Ok(Feedback::Ignore) => {
							_ = done.send((index, Feedback::Ignore));
							return;
						}
						Ok(Feedback::Cancel) | Err(..) => {
							_ = done.send((index, Feedback::Cancel));
							return;
						}
					};

					if let Some(sample) = SamplePair::new(path) {
						done.send((index, Feedback::Use(Ok(sample)))).unwrap();
						return;
					}
				}
			});
		}

		drop(paths);
		drop(done);

		for (index, sample) in receiver {
			match sample {
				Feedback::Use(Ok(sample)) => {
					let id = sample.gui.id;
					arrangement.add_sample(sample);
					samples.insert(index, Feedback::Use(id));
				}
				Feedback::Use(Err(msg)) => daw.try_send(msg).unwrap(),
				Feedback::Ignore => _ = samples.insert(index, Feedback::Ignore),
				Feedback::Cancel => return None,
			}
			current_progress += progress_per_audio;
			daw.try_send(daw::Message::Progress(current_progress))
				.unwrap();
		}

		let mut midi_patterns = HashMap::new();
		for (index, pattern) in reader.iter_midi_patterns() {
			let notes = pattern
				.notes
				.iter()
				.map(|note| MidiNote {
					id: MidiNoteId::unique(),
					key: MidiKey(note.key as u8),
					velocity: note.velocity,
					position: BeatRange::new(
						BeatTime::from_bits(note.position.start),
						BeatTime::from_bits(note.position.end),
					),
				})
				.collect();

			let pattern = MidiPatternPair::from_notes(notes, &pattern.name);
			let id = pattern.gui.id;
			arrangement.add_midi_pattern(pattern);
			midi_patterns.insert(index, id);
		}

		let mut messages = Vec::new();

		let mut ignored_plugins = HashSet::new();

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

			let mut skipped = 0;

			for (i, plugin) in channel.plugins.iter().enumerate() {
				let id = plugin.id();

				let Some(descriptor) = plugin_bundles.iter().find(|d| *d.id == id) else {
					if ignored_plugins.contains(id) {
						skipped += 1;
						continue;
					}

					let (sender, receiver) = oneshot::channel();

					daw.try_send(daw::Message::CantFindPlugin(id.into(), sender.into()))
						.unwrap();

					match receiver.recv() {
						Ok(Feedback::Ignore) => {
							ignored_plugins.insert(id.to_owned());
							skipped += 1;
							continue;
						}
						Ok(Feedback::Cancel) | Err(..) => return None,
					};
				};

				let i = i - skipped;

				messages.push(arrangement_view::Message::PluginAdd(
					node.id,
					descriptor.clone(),
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

				if plugin.active {
					messages.push(arrangement_view::Message::PluginToggleActive(node.id, i));
				}
			}

			Some(())
		};

		let mut tracks = HashMap::new();
		for (index, clips, channel) in reader.iter_tracks() {
			let track = arrangement.tracks().len();
			let id = arrangement.insert_track(arrangement.tracks().len());
			tracks.insert(index, id);
			load_channel(arrangement.node(id), channel)?;

			for clip in clips {
				arrangement.add_clip(
					track,
					match clip {
						proto::Clip::Audio(clip) => {
							let Feedback::Use(sample) = samples.get(&clip.sample)? else {
								continue;
							};
							Clip::Audio(AudioClip {
								id: ClipId::unique(),
								sample: *sample,
								position: clip.position_compat.map_or_else(
									|| {
										OffsetBeatSpan::new(
											BeatSpan::new(
												BeatTime::from_bits(clip.position.position.start),
												SecondsTime::from_bits(clip.position.position.len),
											),
											SecondsTime::from_bits(clip.position.offset),
										)
									},
									|position_compat| {
										OffsetBeatSpan::new(
											BeatSpan::new(
												BeatTime::from_bits(position_compat.position.start),
												BeatTime::from_bits(
													position_compat.position.end
														- position_compat.position.start,
												)
												.to_seconds_time(arrangement.transport()),
											),
											BeatTime::from_bits(position_compat.offset)
												.to_seconds_time(arrangement.transport()),
										)
									},
								),
								volume: clip.volume,
								fade_start: Transition {
									len: SecondsTime::from_bits(clip.fade_start.len),
									p: Point {
										x: clip.fade_start.p.x,
										y: clip.fade_start.p.y,
									},
									symmetric: clip.fade_start.symmetric,
								},
								fade_end: Transition {
									len: SecondsTime::from_bits(clip.fade_end.len),
									p: Point {
										x: clip.fade_end.p.x,
										y: clip.fade_end.p.y,
									},
									symmetric: clip.fade_end.symmetric,
								},
								stretch: clip
									.stretch_compat
									.map_or(clip.stretch, |stretch_compat| {
										f64::from(stretch_compat)
									}),
							})
						}
						proto::Clip::Midi(clip) => Clip::Midi(MidiClip {
							id: ClipId::unique(),
							pattern: *midi_patterns.get(&clip.pattern)?,
							position: OffsetBeatRange::new(
								BeatRange::new(
									BeatTime::from_bits(clip.position.position.start),
									BeatTime::from_bits(clip.position.position.end),
								),
								BeatTime::from_bits(clip.position.offset),
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
		let (index, channel) = iter_channels.next()?;
		load_channel(node, channel)?;
		channels.insert(index, node.id);

		for (index, channel) in iter_channels {
			let id = arrangement.add_channel();
			channels.insert(index, id);
			load_channel(arrangement.node(id), channel)?;
		}

		drop(ignored_plugins);

		for (from, to, mix) in reader.iter_track_to_channel() {
			messages.push(arrangement_view::Message::Connect(
				*tracks.get(&from)?,
				*channels.get(&to)?,
			));

			if mix != 1.0 {
				messages.push(arrangement_view::Message::SetMix(
					*tracks.get(&from)?,
					*channels.get(&to)?,
					mix,
				));
			}
		}

		drop(tracks);

		for (from, to, mix) in reader.iter_channel_to_channel() {
			messages.push(arrangement_view::Message::Connect(
				*channels.get(&from)?,
				*channels.get(&to)?,
			));

			if mix != 1.0 {
				messages.push(arrangement_view::Message::SetMix(
					*channels.get(&from)?,
					*channels.get(&to)?,
					mix,
				));
			}
		}

		drop(channels);

		let view = reader.view();

		drop(reader);

		let project = Project::unique();

		Some(
			Task::done(daw::Message::MergeConfig(config.into(), false))
				.chain(Task::done(daw::Message::ProjectLoaded(
					project,
					Box::new(arrangement).into(),
					view,
				)))
				.chain(Task::batch([
					task.map(arrangement_view::Message::Batch)
						.map(move |message| daw::Message::Arrangement(project, message)),
					messages
						.into_iter()
						.map(Task::done)
						.fold(Task::none(), Task::chain)
						.map(move |message| daw::Message::Arrangement(project, message))
						.chain(Task::done(daw::Message::OpenedFile(Some(path)))),
				])),
		)
	}
}
