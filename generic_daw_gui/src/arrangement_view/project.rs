use super::{ArrangementWrapper, Node};
use crate::{
	arrangement_view::{ArrangementView, LoadStatus, Message, crc},
	clap_host::Message as ClapHostMessage,
	config::Config,
};
use arc_swap::ArcSwap;
use generic_daw_core::{
	self as core, AudioClip, Clip, MidiClip, MidiKey, MidiNote, Mixer, Sample,
	audio_graph::NodeImpl as _,
	clap_host::{MainThreadMessage, PluginBundle, PluginDescriptor},
};
use generic_daw_project::{proto, reader::Reader, writer::Writer};
use iced::{Task, widget::combo_box};
use log::info;
use std::{
	collections::{BTreeMap, HashMap},
	fs::File,
	io::{Read as _, Write as _},
	iter::once,
	path::Path,
	sync::{Arc, mpsc},
};
use walkdir::WalkDir;

impl ArrangementView {
	pub fn save(&mut self, path: &Path) {
		let mut writer = Writer::new(
			u32::from(self.arrangement.rtstate().bpm),
			u32::from(self.arrangement.rtstate().numerator),
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
					key: u32::from(note.key.0),
					velocity: note.velocity as f32,
					start: note.start.into(),
					end: note.end.into(),
				})),
			);
		}

		let mut tracks = HashMap::new();
		for track in self.arrangement.tracks() {
			let node = &self.arrangement.node(track.id).0;
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
						.0
						.plugins
						.iter()
						.map(|plugin| proto::Plugin {
							id: plugin.descriptor.id.to_bytes_with_nul().to_owned(),
							state: self.clap_host.get_state(plugin.id),
						}),
					node.volume,
					node.pan,
				),
			);
		}

		let mut channels = HashMap::new();
		for channel in once(&self.arrangement.master().0).chain(self.arrangement.channels()) {
			channels.insert(
				channel.id,
				writer.push_channel(
					self.arrangement
						.node(channel.id)
						.0
						.plugins
						.iter()
						.map(|plugin| proto::Plugin {
							id: plugin.descriptor.id.to_bytes_with_nul().to_owned(),
							state: self.clap_host.get_state(plugin.id),
						}),
					channel.volume,
					channel.pan,
				),
			);
		}

		for track in self.arrangement.tracks() {
			for connection in &self.arrangement.node(track.id).1 {
				writer.connect_track_to_channel(tracks[&track.id], channels[&connection]);
			}
		}

		for channel in self.arrangement.channels() {
			for connection in &self.arrangement.node(channel.id).1 {
				writer.connect_channel_to_channel(channels[&channel.id], channels[&connection]);
			}
		}

		File::create(path)
			.unwrap()
			.write_all(&writer.finalize())
			.unwrap();
	}

	pub fn load(
		&mut self,
		path: &Path,
		config: &Config,
		plugin_bundles: &BTreeMap<PluginDescriptor, PluginBundle>,
	) -> Option<Task<Message>> {
		info!("loading project {}", path.display());

		let mut gdp = Vec::new();
		File::open(path).ok()?.read_to_end(&mut gdp).ok()?;
		let reader = Reader::new(&gdp)?;

		let mut futs = Vec::new();

		let (mut arrangement, task) = ArrangementWrapper::create(config);
		futs.push(task.map(Message::Batch));

		arrangement.set_bpm(reader.rtstate().bpm as u16);
		arrangement.set_numerator(reader.rtstate().numerator as u8);

		let mut audios = HashMap::new();
		let mut midis = HashMap::new();

		let (sender, receiver) = mpsc::channel();
		std::thread::scope(|s| {
			for (idx, audio) in reader.iter_audios() {
				let sender = sender.clone();
				let sample_rate = arrangement.rtstate().sample_rate;
				s.spawn(move || {
					let audio = config
						.sample_paths
						.iter()
						.flat_map(WalkDir::new)
						.flatten()
						.filter(|dir_entry| dir_entry.file_type().is_file())
						.filter(|dir_entry| {
							dir_entry
								.path()
								.file_name()
								.is_some_and(|name| *name == *audio.name)
						})
						.filter(|dir_entry| {
							File::open(dir_entry.path()).is_ok_and(|file| crc(file) == audio.crc)
						})
						.find_map(|dir_entry| Sample::create(dir_entry.path().into(), sample_rate))
						.map(|sample| (audio.crc, sample));

					sender.send((idx, audio)).unwrap();
				});
			}

			for (idx, notes) in reader.iter_midis() {
				let pattern = notes
					.notes
					.iter()
					.map(|note| MidiNote {
						channel: 0,
						key: MidiKey(note.key as u8),
						velocity: f64::from(note.velocity),
						start: note.start.into(),
						end: note.end.into(),
					})
					.collect();

				midis.insert(idx, Arc::new(ArcSwap::new(Arc::new(pattern))));
			}
		});

		while let Ok((idx, audio)) = receiver.try_recv() {
			audios.insert(idx, audio?);
		}

		let mut load_channel = |node: &Node, channel: &proto::Channel| {
			futs.push(Task::done(Message::ChannelVolumeChanged(
				node.id,
				channel.volume,
			)));
			futs.push(Task::done(Message::ChannelPanChanged(node.id, channel.pan)));

			for plugin in &channel.plugins {
				futs.push(Task::done(Message::PluginLoad(
					plugin_bundles
						.keys()
						.find(|d| *d.id == *plugin.id())?
						.clone(),
					plugin.state.clone().map(|state| state.into_boxed_slice()),
				)));
			}

			Some(())
		};

		let mut tracks = HashMap::new();
		for (idx, clips, channel) in reader.iter_tracks() {
			let mut track = core::Track::default();

			for clip in clips {
				track.clips.push(match clip {
					proto::Clip::Audio(audio) => {
						let clip = AudioClip::create(
							audios.get(&audio.audio)?.1.clone(),
							arrangement.rtstate(),
						);
						clip.position.move_to(audio.position.start.into());
						clip.position.trim_end_to(audio.position.end.into());
						clip.position.trim_start_to(audio.position.offset.into());
						Clip::Audio(clip)
					}
					proto::Clip::Midi(midi) => {
						let clip = MidiClip::create(midis.get(&midi.midi)?.clone());
						clip.position.move_to(midi.position.start.into());
						clip.position.trim_end_to(midi.position.end.into());
						clip.position.trim_start_to(midi.position.offset.into());
						Clip::Midi(clip)
					}
				});
			}

			let id = track.id();
			tracks.insert(idx, id);
			arrangement.push_track(track);
			load_channel(&arrangement.node(id).0, channel)?;
		}

		let mut channels = HashMap::new();
		let mut iter_channels = reader.iter_channels();

		let node = &arrangement.master().0;
		let (idx, channel) = iter_channels.next()?;
		load_channel(node, channel)?;
		channels.insert(idx, node.id);

		for (idx, channel) in iter_channels {
			let mixer_node = Mixer::default();
			let id = mixer_node.id();
			channels.insert(idx, id);
			arrangement.push_channel(mixer_node);
			load_channel(&arrangement.node(id).0, channel)?;
		}

		for (from, to) in reader.iter_connections_track_channel() {
			futs.push(Task::perform(
				arrangement.request_connect(*channels.get(&to)?, *tracks.get(&from)?),
				|con| Message::ConnectSucceeded(con.unwrap()),
			));
		}

		for (from, to) in reader.iter_connections_channel_channel() {
			futs.push(Task::perform(
				arrangement.request_connect(*channels.get(&from)?, *channels.get(&to)?),
				|con| Message::ConnectSucceeded(con.unwrap()),
			));
		}

		info!("loaded project {}", path.display());

		futs.extend(self.clear());

		self.plugin_descriptors = combo_box::State::new(plugin_bundles.keys().cloned().collect());
		self.arrangement = arrangement;
		self.audios.clear();
		self.audios.extend(audios.values().map(|(crc, audio)| {
			(
				audio.path.clone(),
				LoadStatus::Loaded(*crc, Arc::downgrade(audio)),
			)
		}));
		self.midis.clear();
		self.midis.extend(midis.values().map(Arc::downgrade));

		Some(Task::batch(futs))
	}

	pub fn unload(
		&mut self,
		config: &Config,
		plugin_bundles: &BTreeMap<PluginDescriptor, PluginBundle>,
	) -> Task<Message> {
		let (arrangement, task) = ArrangementWrapper::create(config);

		let futs = Task::batch(self.clear().chain(once(task.map(Message::Batch))));

		self.plugin_descriptors = combo_box::State::new(plugin_bundles.keys().cloned().collect());
		self.arrangement = arrangement;
		self.audios.clear();
		self.midis.clear();

		futs
	}

	fn clear(&mut self) -> impl Iterator<Item = Task<Message>> {
		self.loading.clear();
		self.audios.clear();
		self.midis.clear();
		self.recording = None;
		self.soloed_track = None;
		self.selected_channel = None;
		self.arrangement.plugins().map(|id| {
			self.clap_host
				.update(ClapHostMessage::MainThread(
					id,
					MainThreadMessage::GuiClosed,
				))
				.map(Message::ClapHost)
		})
	}
}
