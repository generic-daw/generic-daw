use crate::{
	arrangement_view::{Message, node::Node, recording::Recording, sample::SamplePair},
	daw::{RECORDINGS_DIR, format_now},
	stylefns::{container_with_radius, weak_bordered_box},
};
use generic_daw_core::{Channels, Clip, NodeId, Transport, time::BeatTime};
use iced::{
	Element,
	widget::{button, container, space, text, tooltip},
};
use log::warn;
use rtrb::Consumer;
use std::num::NonZero;

#[derive(Debug)]
pub struct Input {
	pub consumer: Consumer<[f32; 2]>,
	pub channels: Channels,
	pub recording: Option<Recording>,
}

#[derive(Debug)]
pub struct Track {
	pub id: NodeId,
	pub clips: Vec<Clip>,
	pub input: Option<Input>,
}

impl Track {
	pub fn new(id: NodeId) -> Self {
		Self {
			id,
			clips: Vec::new(),
			input: None,
		}
	}

	pub fn len(&self, transport: &Transport) -> BeatTime {
		self.clips
			.iter()
			.map(|clip| clip.end(transport))
			.max()
			.unwrap_or_default()
	}

	pub fn interrupted(&mut self) -> Option<(BeatTime, SamplePair)> {
		Some(self.input.as_mut()?.recording.take()?.finalize())
	}

	pub fn recorded(&mut self, samples: &mut [[f32; 2]], transport: &Transport, track: usize) {
		let Some(input) = &mut self.input else {
			return;
		};

		if let (_, t) = input.consumer.pop_partial_slice(samples)
			&& !t.is_empty()
		{
			warn!("empty ring buffer");
			t.fill([0.0; 2]);
		}

		input
			.recording
			.get_or_insert_with(|| {
				Recording::new(
					RECORDINGS_DIR
						.join(format!("{} T{}.wav", format_now(), track + 1))
						.into(),
					transport,
				)
			})
			.recorded(samples);
	}

	pub fn recording_button<'a>(
		&'a self,
		enabled: bool,
		transport: &'a Transport,
	) -> Element<'a, Message> {
		let recording_button = button(container(space().width(10).height(10)).style(
			container_with_radius(
				move |t| {
					container::background(
						self.input
							.as_ref()
							.map_or_else(
								|| {
									if enabled {
										t.palette().primary
									} else {
										t.palette().secondary
									}
								},
								|input| {
									if input.channels.fits_in(transport.input_channels) {
										t.palette().danger
									} else {
										t.palette().warning
									}
								},
							)
							.base
							.text,
					)
				},
				f32::INFINITY,
			),
		))
		.padding(2.5)
		.style(self.input.as_ref().map_or(
			if enabled {
				button::primary
			} else {
				button::secondary
			},
			|input| {
				if input.channels.fits_in(transport.output_channels.get()) {
					button::danger
				} else {
					button::warning
				}
			},
		));

		if let Some(channels) = NonZero::new(transport.input_channels) {
			Node::input_context_menu(
				self.id,
				recording_button.on_press(Message::InputChangeChannels(
					self.id,
					self.input
						.as_ref()
						.map_or_else(|| Some(Channels::base(channels)), |_| None),
				)),
				self.input.as_ref().map(|input| input.channels),
				transport,
			)
		} else {
			tooltip(
				recording_button,
				container(text("No available input device").line_height(1.0))
					.padding(3)
					.style(container_with_radius(weak_bordered_box, 2)),
				tooltip::Position::Bottom,
			)
			.into()
		}
	}
}
