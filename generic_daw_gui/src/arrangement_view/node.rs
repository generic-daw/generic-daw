use crate::{
	arrangement_view::{Message, Tab, format_pan, plugin::Plugin},
	components::context_menu_entry,
	icons::{
		arrow_up_down, between_horizontal_start, between_vertical_start,
		chevrons_left_right_ellipsis, circle_ellipsis, copy, power, power_off, rotate_ccw,
		snowflake,
	},
	stylefns::{container_with_radius, weaker_bordered_box},
};
use generic_daw_core::{Channels, NodeId, PanMode, Transport, Utility};
use generic_daw_widget::{context_menu::ContextMenu, knob::Knob, peak_meter};
use iced::{
	Alignment::Center,
	Element, Fill, padding,
	widget::{self, column, container, radio, row, rule, space, text, value},
};
use std::{iter::once, time::Instant};
use utils::NoDebug;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeType {
	Master,
	Channel,
	Track,
}

#[derive(Debug)]
pub struct Node {
	pub ty: NodeType,
	pub id: NodeId,
	pub widget_id: widget::Id,
	pub plugins: Vec<Plugin>,
	pub utility: Utility,
	pub enabled: bool,
	pub bypassed: bool,
	pub output: Option<Channels>,
	pub peaks: NoDebug<[peak_meter::State; 2]>,
	pub polyphony: usize,
}

impl Node {
	pub fn new(ty: NodeType, id: NodeId, output: Option<Channels>) -> Self {
		Self {
			ty,
			id,
			widget_id: widget::Id::unique(),
			plugins: Vec::new(),
			utility: Utility {
				volume: 1.0,
				pan: PanMode::Stereo(0.0),
			},
			enabled: true,
			bypassed: false,
			output,
			peaks: [peak_meter::State::default(), peak_meter::State::default()].into(),
			polyphony: 0,
		}
	}

	pub fn update(&mut self, peaks: [f32; 2], now: Instant) {
		self.peaks[0].update(peaks[0], now);
		self.peaks[1].update(peaks[1], now);
	}

	pub fn main_context_menu<'a>(
		&'a self,
		content: impl Into<Element<'a, Message>>,
		tab: Tab,
	) -> Element<'a, Message> {
		ContextMenu::new(
			content,
			container(column![
				context_menu_entry(
					if tab == Tab::Mixer {
						between_vertical_start()
					} else {
						between_horizontal_start()
					},
					"Insert",
					""
				)
				.on_press_maybe(match self.ty {
					NodeType::Master => None,
					NodeType::Track => Some(Message::TrackInsert(self.id)),
					NodeType::Channel => Some(Message::ChannelInsert(self.id)),
				}),
				context_menu_entry(
					copy(),
					"Duplicate",
					if tab == Tab::Mixer { "Ctrl-D" } else { "" }
				)
				.on_press_maybe(match self.ty {
					NodeType::Master => None,
					NodeType::Track => Some(Message::TrackDuplicate(self.id)),
					NodeType::Channel => Some(Message::ChannelDuplicate(self.id)),
				}),
				container(rule::horizontal(1)).padding(padding::horizontal(5)),
				if self.bypassed {
					context_menu_entry(power_off(), "Engage FX", "")
				} else {
					context_menu_entry(power(), "Bypass FX", "")
				}
				.on_press(Message::ChannelToggleBypassed(self.id)),
				(tab == Tab::Playlist).then(|| context_menu_entry(snowflake(), "Freeze", "")
					.on_press(Message::Freeze(self.id))),
				container(rule::horizontal(1)).padding(padding::horizontal(5)),
				context_menu_entry(
					arrow_up_down(),
					"Invert polarity",
					if tab == Tab::Mixer { "Alt-I" } else { "" }
				)
				.on_press(Message::ChannelVolumeChanged(self.id, -self.utility.volume)),
				match self.utility.pan {
					PanMode::Stereo(..) =>
						context_menu_entry(chevrons_left_right_ellipsis(), "Split stereo pan", "")
							.on_press(Message::ChannelPanChanged(
								self.id,
								PanMode::SplitStereo(-1.0, 1.0)
							)),
					PanMode::SplitStereo(..) =>
						context_menu_entry(circle_ellipsis(), "Stereo pan", "")
							.on_press(Message::ChannelPanChanged(self.id, PanMode::Stereo(0.0))),
				}
			])
			.width(if tab == Tab::Mixer { 180 } else { 160 })
			.style(container_with_radius(weaker_bordered_box, 5)),
		)
		.into()
	}

	pub fn volume_context_menu<'a>(
		&'a self,
		content: impl Into<Element<'a, Message>>,
		tab: Tab,
	) -> Element<'a, Message> {
		ContextMenu::new(
			content,
			container(column![
				context_menu_entry(rotate_ccw(), "Reset", "Ctrl-Click")
					.on_press(Message::ChannelVolumeChanged(self.id, 1.0)),
				container(rule::horizontal(1)).padding(padding::horizontal(5)),
				context_menu_entry(
					arrow_up_down(),
					"Invert polarity",
					if tab == Tab::Mixer { "Alt-I" } else { "" }
				)
				.on_press(Message::ChannelVolumeChanged(self.id, -self.utility.volume)),
			])
			.width(if tab == Tab::Mixer { 180 } else { 160 })
			.style(container_with_radius(weaker_bordered_box, 5)),
		)
		.into()
	}

	pub fn pan_knob(&self, radius: f32, enabled: bool) -> Element<'_, Message> {
		const RADIUS: f32 = 0.571_595_13; // 1.95 - sqrt(1.9)
		const SPACING: f32 = -0.286_380_5; // 2 * (2 * sqrt(1.9) - 2.9)

		match self.utility.pan {
			PanMode::Stereo(pan) => ContextMenu::new(
				Knob::new(-1.0..=1.0, pan, |pan| {
					Message::ChannelPanChanged(self.id, PanMode::Stereo(pan))
				})
				.origin(0.0)
				.default(0.0)
				.radius(radius)
				.enabled(enabled)
				.tooltip(format_pan(pan)),
				container(column![
					context_menu_entry(rotate_ccw(), "Reset", "Ctrl-Click")
						.on_press(Message::ChannelPanChanged(self.id, PanMode::Stereo(0.0))),
					container(rule::horizontal(1)).padding(padding::horizontal(5)),
					context_menu_entry(chevrons_left_right_ellipsis(), "Split stereo pan", "")
						.on_press(Message::ChannelPanChanged(
							self.id,
							PanMode::SplitStereo(-1.0, 1.0)
						)),
				])
				.width(160)
				.style(container_with_radius(weaker_bordered_box, 5)),
			)
			.into(),
			PanMode::SplitStereo(l, r) => ContextMenu::new(
				row![
					container(ContextMenu::new(
						Knob::new(-1.0..=1.0, l, move |l| {
							Message::ChannelPanChanged(self.id, PanMode::SplitStereo(l, r))
						})
						.origin(0.0)
						.default(-1.0)
						.radius(radius * RADIUS)
						.enabled(enabled)
						.tooltip(format_pan(l)),
						container(column![
							context_menu_entry(rotate_ccw(), "Reset", "Ctrl-Click").on_press(
								Message::ChannelPanChanged(self.id, PanMode::SplitStereo(-1.0, r))
							),
							container(rule::horizontal(1)).padding(padding::horizontal(5)),
							context_menu_entry(circle_ellipsis(), "Stereo pan", "").on_press(
								Message::ChannelPanChanged(self.id, PanMode::Stereo(0.0))
							),
						])
						.width(160)
						.style(container_with_radius(weaker_bordered_box, 5))
					),)
					.align_top(Fill),
					container(ContextMenu::new(
						Knob::new(-1.0..=1.0, r, move |r| {
							Message::ChannelPanChanged(self.id, PanMode::SplitStereo(l, r))
						})
						.origin(0.0)
						.default(1.0)
						.radius(radius * RADIUS)
						.enabled(enabled)
						.tooltip(format_pan(r)),
						container(column![
							context_menu_entry(rotate_ccw(), "Reset", "Ctrl-Click").on_press(
								Message::ChannelPanChanged(self.id, PanMode::SplitStereo(l, 1.0))
							),
							container(rule::horizontal(1)).padding(padding::horizontal(5)),
							context_menu_entry(circle_ellipsis(), "Stereo pan", "").on_press(
								Message::ChannelPanChanged(self.id, PanMode::Stereo(0.0))
							),
						])
						.width(160)
						.style(container_with_radius(weaker_bordered_box, 5))
					))
					.align_bottom(Fill)
				]
				.spacing(radius * SPACING)
				.width(2.0 * radius)
				.height(1.8 * radius),
				container(
					context_menu_entry(circle_ellipsis(), "Stereo pan", "")
						.on_press(Message::ChannelPanChanged(self.id, PanMode::Stereo(0.0))),
				)
				.width(160)
				.style(container_with_radius(weaker_bordered_box, 5)),
			)
			.into(),
		}
	}

	pub fn input_context_menu<'a>(
		id: NodeId,
		content: impl Into<Element<'a, Message>>,
		input: Option<Channels>,
		transport: &Transport,
	) -> Element<'a, Message> {
		ContextMenu::new(
			content,
			input.map(|input| {
				container(
					row![
						column(once(space().height(15).into()).chain(
							(0..transport.input_channels).map(|channel| {
								container(value(channel + 1).size(13).line_height(1.0))
									.padding(1)
									.into()
							})
						))
						.spacing(5)
						.align_x(Center),
						column(
							once(
								container(text("L").size(13).line_height(1.0))
									.padding(1)
									.into(),
							)
							.chain((0..transport.input_channels).map(|channel| {
								radio("", channel, Some(input.left), |_| {
									Message::InputChangeChannels(id, Some(input.left(channel)))
								})
								.size(15)
								.text_size(1)
								.spacing(0)
								.into()
							})),
						)
						.spacing(5)
						.align_x(Center),
						column(
							once(
								container(text("R").size(13).line_height(1.0))
									.padding(1)
									.into(),
							)
							.chain((0..transport.input_channels).map(|channel| {
								radio("", channel, Some(input.right), |_| {
									Message::InputChangeChannels(id, Some(input.right(channel)))
								})
								.size(15)
								.text_size(1)
								.spacing(0)
								.into()
							})),
						)
						.spacing(5)
						.align_x(Center),
					]
					.spacing(5),
				)
				.padding(5)
				.style(container_with_radius(weaker_bordered_box, 5))
			}),
		)
		.into()
	}

	pub fn output_context_menu<'a>(
		id: NodeId,
		content: impl Into<Element<'a, Message>>,
		output: Option<Channels>,
		transport: &Transport,
	) -> Element<'a, Message> {
		ContextMenu::new(
			content,
			output.map(|output| {
				container(
					column![
						row(once(space().width(15).into()).chain(
							(0..transport.output_channels.get()).map(|channel| container(
								value(channel + 1).size(13).line_height(1.0)
							)
							.padding(1)
							.center_x(15)
							.into())
						))
						.spacing(5),
						row(once(
							container(text("L").size(13).line_height(1.0))
								.padding(1)
								.center_x(15)
								.into(),
						)
						.chain((0..transport.output_channels.get()).map(|channel| {
							radio("", channel, Some(output.left), |_| {
								Message::OutputChangeChannels(id, Some(output.left(channel)))
							})
							.size(15)
							.text_size(1)
							.spacing(0)
							.into()
						})))
						.spacing(5),
						row(once(
							container(text("R").size(13).line_height(1.0))
								.padding(1)
								.center_x(15)
								.into(),
						)
						.chain((0..transport.output_channels.get()).map(|channel| {
							radio("", channel, Some(output.right), |_| {
								Message::OutputChangeChannels(id, Some(output.right(channel)))
							})
							.size(15)
							.text_size(1)
							.spacing(0)
							.into()
						})))
						.spacing(5),
					]
					.spacing(5),
				)
				.padding(5)
				.style(container_with_radius(weaker_bordered_box, 5))
			}),
		)
		.into()
	}
}
