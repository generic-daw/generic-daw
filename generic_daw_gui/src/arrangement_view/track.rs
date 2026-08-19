use crate::{
	arrangement_view::{
		Message, MidiRecording, audio_recording::AudioRecording, midi_pattern::MidiPatternPair,
		sample::SamplePair,
	},
	components::icon_button,
	daw::{RECORDINGS_DIR, format_now},
	icons::{keyboard_music, mic, square_arrow_right_enter},
	stylefns::{button_with_radius, container_with_radius, weaker_bordered_box},
};
use generic_daw_core::{
	Channels, Clip, NodeId, TimedMidiAction, Transport,
	time::{BeatRange, BeatTime},
};
use generic_daw_widget::menu::Menu;
use iced::{
	Center, Element, Fit, Shrink,
	widget::{
		Button, button, checkbox, column, container, radio, row, scrollable, space, text, value,
	},
};
use log::warn;
use rtrb::Consumer;
use std::{cell::LazyCell, mem::MaybeUninit};

#[derive(Debug)]
pub struct Track {
	pub id: NodeId,
	pub clips: Vec<Clip>,
	pub input: Channels,
	pub audio_consumer: Option<Consumer<[f32; 2]>>,
	pub audio_recording: Option<AudioRecording>,
	pub midi_consumer: Option<Consumer<TimedMidiAction<BeatTime>>>,
	pub midi_recording: Option<MidiRecording>,
}

impl Track {
	pub fn new(id: NodeId, input: Channels) -> Self {
		Self {
			id,
			clips: Vec::new(),
			input,
			audio_consumer: None,
			audio_recording: None,
			midi_consumer: None,
			midi_recording: None,
		}
	}

	pub fn len(&self, transport: &Transport) -> BeatTime {
		self.clips
			.iter()
			.map(|clip| clip.end(transport))
			.max()
			.unwrap_or_default()
	}

	pub fn audio_recorded(
		&mut self,
		samples: &mut [[f32; 2]],
		transport: &Transport,
		name: &LazyCell<String, impl FnOnce() -> String>,
	) {
		let Some(audio_consumer) = &mut self.audio_consumer else {
			return;
		};

		if let (_, rest) = audio_consumer.pop_partial_slice(samples)
			&& !rest.is_empty()
		{
			warn!("empty ring buffer");
			rest.fill([0.0; 2]);
		}

		if self.audio_recording.is_none() {
			self.audio_recording = AudioRecording::new(
				RECORDINGS_DIR
					.join(format!("{} {}.wav", **name, format_now()))
					.into(),
				transport,
			)
			.inspect_err(|err| warn!("{err}"))
			.ok();
		}

		if let Some(audio_recording) = &mut self.audio_recording {
			audio_recording.recorded(samples);
		}
	}

	pub fn audio_finalize(&mut self) -> Option<(BeatTime, SamplePair)> {
		if let Some(audio_consumer) = &self.audio_consumer
			&& audio_consumer.is_abandoned()
			&& audio_consumer.is_empty()
		{
			self.audio_consumer = None;
		}

		Some(self.audio_recording.take()?.finalize())
	}

	pub fn midi_recorded(
		&mut self,
		actions: &mut [MaybeUninit<TimedMidiAction<BeatTime>>],
		frames: usize,
		transport: &Transport,
		name: &LazyCell<String, impl FnOnce() -> String>,
	) {
		let Some(midi_consumer) = &mut self.midi_consumer else {
			return;
		};

		let actions = midi_consumer.pop_partial_slice_uninit(actions).0;

		self.midi_recording
			.get_or_insert_with(|| {
				MidiRecording::new(format!("{} {}", **name, format_now()).into(), transport)
			})
			.recorded(actions, frames);
	}

	pub fn midi_finalize(&mut self, transport: &Transport) -> Option<(BeatRange, MidiPatternPair)> {
		if let Some(midi_consumer) = &self.midi_consumer
			&& midi_consumer.is_abandoned()
			&& midi_consumer.is_empty()
		{
			self.midi_consumer = None;
		}

		Some(self.midi_recording.take()?.finalize(transport))
	}

	fn arm_button<'a>(enabled: bool, armed: bool, fits: bool) -> Button<'a, Message> {
		button(
			container(space().width(10).height(10)).style(container_with_radius(
				move |t| {
					container::background(
						if armed {
							if fits {
								t.palette().danger
							} else {
								t.palette().warning
							}
						} else if enabled {
							t.palette().primary
						} else {
							t.palette().secondary
						}
						.base
						.text,
					)
				},
				f32::INFINITY,
			)),
		)
		.padding(2.5)
		.style(button_with_radius(
			if armed {
				if fits {
					button::danger
				} else {
					button::warning
				}
			} else if enabled {
				button::primary
			} else {
				button::secondary
			},
			0,
		))
	}

	fn monitor_button<'a>(enabled: bool, armed: bool, fits: bool) -> Button<'a, Message> {
		icon_button(
			square_arrow_right_enter(),
			if armed {
				if fits {
					button::danger
				} else {
					button::warning
				}
			} else if enabled {
				button::primary
			} else {
				button::secondary
			},
		)
	}

	pub fn inputs_toolbar<'a>(
		&'a self,
		enabled: bool,
		transport: &'a Transport,
	) -> Element<'a, Message> {
		column![
			row![
				Menu::new(mic().size(13.0), move || container(
					column![
						row![
							space::horizontal().height(15),
							container(text("L").size(13).line_height(1.0))
								.center(15)
								.padding(1),
							container(text("R").size(13).line_height(1.0))
								.center(15)
								.padding(1)
						]
						.spacing(5),
						scrollable(
							row![
								column((0..transport.input_channels).map(|channel| {
									container(value(channel + 1).size(13).line_height(1.0))
										.padding(1)
										.into()
								}))
								.spacing(5)
								.align_x(Center),
								column((0..transport.input_channels).map(|channel| {
									radio(channel, Some(self.input.left), |_| {
										Message::InputChangeChannels(
											self.id,
											self.input.left(channel),
										)
									})
									.size(15)
									.into()
								}))
								.spacing(5),
								column((0..transport.input_channels).map(|channel| {
									radio(channel, Some(self.input.right), |_| {
										Message::InputChangeChannels(
											self.id,
											self.input.right(channel),
										)
									})
									.size(15)
									.into()
								}))
								.spacing(5),
							]
							.spacing(5),
						)
						.direction(scrollable::Direction::Vertical(
							scrollable::Scrollbar::hidden(),
						))
						.height(Fit.max(315))
					]
					.width(Shrink)
					.spacing(5)
				)
				.padding(5)
				.style(container_with_radius(weaker_bordered_box, 5))
				.into())
				.padding(1)
				.style(button_with_radius(
					if enabled {
						button::primary
					} else {
						button::secondary
					},
					0
				)),
				Self::monitor_button(
					enabled,
					self.input.enable_audio,
					self.input.fits_in(transport.input_channels),
				)
				.on_press(Message::InputChangeChannels(
					self.id,
					self.input.enable_audio(!self.input.enable_audio)
				)),
				Self::arm_button(
					enabled,
					self.audio_consumer.is_some(),
					self.input.fits_in(transport.input_channels),
				)
				.on_press(Message::InputToggleAudioRecording(self.id)),
			]
			.spacing(5)
			.height(Shrink),
			row![
				Menu::new(keyboard_music().size(13.0), move || container(
					scrollable(
						row![
							column((0..16).map(|channel| {
								container(value(channel + 1).size(13).line_height(1.0))
									.padding(1)
									.into()
							}))
							.spacing(5)
							.align_x(Center),
							column((0..16).map(|channel| {
								checkbox(self.input.midi & (1 << channel) != 0)
									.on_toggle(move |_| {
										Message::InputChangeChannels(
											self.id,
											self.input.midi(self.input.midi ^ (1 << channel)),
										)
									})
									.size(15)
									.into()
							}))
							.spacing(5)
							.align_x(Center),
						]
						.spacing(5)
					)
					.direction(scrollable::Direction::Vertical(
						scrollable::Scrollbar::hidden(),
					))
				)
				.padding(5)
				.style(container_with_radius(weaker_bordered_box, 5))
				.into())
				.padding(1)
				.style(button_with_radius(
					if enabled {
						button::primary
					} else {
						button::secondary
					},
					0
				)),
				Self::monitor_button(enabled, self.input.enable_midi, true).on_press(
					Message::InputChangeChannels(
						self.id,
						self.input.enable_midi(!self.input.enable_midi)
					)
				),
				Self::arm_button(enabled, self.midi_consumer.is_some(), true)
					.on_press(Message::InputToggleMidiRecording(self.id)),
			]
			.spacing(5)
			.height(Shrink),
		]
		.spacing(5)
		.into()
	}
}
