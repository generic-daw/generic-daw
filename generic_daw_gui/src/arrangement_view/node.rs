use crate::{
	arrangement_view::{Message, Tab, format_pan, plugin::Plugin},
	components::menu_entry,
	icons::{
		arrow_up_down, between_horizontal_start, between_vertical_start,
		chevrons_left_right_ellipsis, circle_ellipsis, copy, power, power_off, replace, rotate_ccw,
	},
	stylefns::{container_with_radius, weaker_bordered_box},
};
use generic_daw_core::{Channels, NodeId, PanMode, Transport, Utility};
use generic_daw_widget::{context_menu::ContextMenu, knob::Knob, peak_meter};
use iced::{
	Center, Element, Fill, Fit, padding,
	widget::{self, checkbox, column, container, radio, row, rule, scrollable, space, text, value},
};
use std::{collections::BTreeMap, sync::Arc, time::Instant};

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
	pub name: Arc<str>,
	pub widget_id: widget::Id,
	pub plugins: Vec<Plugin>,
	pub utility: Utility,
	pub enabled: bool,
	pub bypassed: bool,
	pub outgoing: BTreeMap<NodeId, f32>,
	pub output: Channels,
	pub peaks: [peak_meter::State; 2],
	pub polyphony: usize,
}

impl Node {
	pub fn new(ty: NodeType, id: NodeId, output: Channels) -> Self {
		Self {
			ty,
			id,
			name: Arc::default(),
			widget_id: widget::Id::unique(),
			plugins: Vec::new(),
			utility: Utility::default(),
			enabled: true,
			bypassed: false,
			outgoing: BTreeMap::new(),
			output,
			peaks: [peak_meter::State::default(), peak_meter::State::default()],
			polyphony: 0,
		}
	}

	pub fn update(&mut self, peaks: [f32; 2], now: Instant) {
		self.peaks[0].update(peaks[0], now);
		self.peaks[1].update(peaks[1], now);
	}

	pub fn main_context_menu(&self, tab: Tab) -> Element<'_, Message> {
		container(column![
			menu_entry(
				if tab == Tab::Mixer {
					between_vertical_start()
				} else {
					between_horizontal_start()
				},
				"Insert after",
				""
			)
			.on_press_maybe(match self.ty {
				NodeType::Master => None,
				NodeType::Channel => Some(Message::ChannelInsert(self.id)),
				NodeType::Track => Some(Message::TrackInsert(self.id)),
			}),
			menu_entry(
				copy(),
				"Duplicate",
				if tab == Tab::Mixer { "Ctrl-D" } else { "" }
			)
			.on_press_maybe(match self.ty {
				NodeType::Master => None,
				NodeType::Channel => Some(Message::ChannelDuplicate(self.id)),
				NodeType::Track => Some(Message::TrackDuplicate(self.id)),
			}),
			(tab == Tab::Mixer).then(|| menu_entry(
				replace(),
				if self.ty == NodeType::Channel {
					"Convert to track"
				} else {
					"Convert to channel"
				},
				""
			)
			.on_press_maybe((self.ty != NodeType::Master).then_some(Message::ToggleKind(self.id)))),
			container(rule::horizontal(1)).padding(padding::horizontal(5)),
			if self.bypassed {
				menu_entry(power_off(), "Engage FX", "")
			} else {
				menu_entry(power(), "Bypass FX", "")
			}
			.on_press(Message::ChannelToggleBypassed(self.id)),
			container(rule::horizontal(1)).padding(padding::horizontal(5)),
			menu_entry(
				arrow_up_down(),
				"Invert polarity",
				if tab == Tab::Mixer { "Alt-I" } else { "" }
			)
			.on_press(Message::ChannelVolumeChanged(self.id, -self.utility.volume)),
			match self.utility.pan {
				PanMode::Stereo(..) =>
					menu_entry(chevrons_left_right_ellipsis(), "Split stereo pan", "").on_press(
						Message::ChannelPanChanged(self.id, PanMode::SplitStereo(-1.0, 1.0))
					),
				PanMode::SplitStereo(..) => menu_entry(circle_ellipsis(), "Stereo pan", "")
					.on_press(Message::ChannelPanChanged(self.id, PanMode::Stereo(0.0))),
			}
		])
		.width(180)
		.style(container_with_radius(weaker_bordered_box, 5))
		.into()
	}

	pub fn volume_context_menu(&self, tab: Tab) -> Element<'_, Message> {
		container(column![
			menu_entry(rotate_ccw(), "Reset", "Ctrl-Click")
				.on_press(Message::ChannelVolumeChanged(self.id, 1.0)),
			container(rule::horizontal(1)).padding(padding::horizontal(5)),
			menu_entry(
				arrow_up_down(),
				"Invert polarity",
				if tab == Tab::Mixer { "Alt-I" } else { "" }
			)
			.on_press(Message::ChannelVolumeChanged(self.id, -self.utility.volume)),
		])
		.width(if tab == Tab::Mixer { 180 } else { 160 })
		.style(container_with_radius(weaker_bordered_box, 5))
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
				move || {
					container(column![
						menu_entry(rotate_ccw(), "Reset", "Ctrl-Click")
							.on_press(Message::ChannelPanChanged(self.id, PanMode::Stereo(0.0))),
						container(rule::horizontal(1)).padding(padding::horizontal(5)),
						menu_entry(chevrons_left_right_ellipsis(), "Split stereo pan", "")
							.on_press(Message::ChannelPanChanged(
								self.id,
								PanMode::SplitStereo(-1.0, 1.0)
							)),
					])
					.width(160)
					.style(container_with_radius(weaker_bordered_box, 5))
					.into()
				},
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
						move || container(column![
							menu_entry(rotate_ccw(), "Reset", "Ctrl-Click").on_press(
								Message::ChannelPanChanged(self.id, PanMode::SplitStereo(-1.0, r))
							),
							container(rule::horizontal(1)).padding(padding::horizontal(5)),
							menu_entry(circle_ellipsis(), "Stereo pan", "").on_press(
								Message::ChannelPanChanged(self.id, PanMode::Stereo(0.0))
							),
						])
						.width(160)
						.style(container_with_radius(weaker_bordered_box, 5))
						.into()
					))
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
						move || container(column![
							menu_entry(rotate_ccw(), "Reset", "Ctrl-Click").on_press(
								Message::ChannelPanChanged(self.id, PanMode::SplitStereo(l, 1.0))
							),
							container(rule::horizontal(1)).padding(padding::horizontal(5)),
							menu_entry(circle_ellipsis(), "Stereo pan", "").on_press(
								Message::ChannelPanChanged(self.id, PanMode::Stereo(0.0))
							),
						])
						.width(160)
						.style(container_with_radius(weaker_bordered_box, 5))
						.into()
					))
					.align_bottom(Fill)
				]
				.spacing(radius * SPACING)
				.width(2.0 * radius)
				.height(1.8 * radius),
				move || {
					container(
						menu_entry(circle_ellipsis(), "Stereo pan", "")
							.on_press(Message::ChannelPanChanged(self.id, PanMode::Stereo(0.0))),
					)
					.width(160)
					.style(container_with_radius(weaker_bordered_box, 5))
					.into()
				},
			)
			.into(),
		}
	}

	pub fn audio_output_context_menu(&self, transport: &Transport) -> Element<'_, Message> {
		container(
			row![
				column![
					space().width(15).height(15),
					container(text("L").size(13).line_height(1.0)).padding(1),
					container(text("R").size(13).line_height(1.0)).padding(1),
				]
				.align_x(Center)
				.spacing(5),
				scrollable(
					row((0..transport.output_channels.get()).map(|channel| {
						column![
							container(value(channel + 1).size(13).line_height(1.0)).padding(1),
							radio(channel, Some(self.output.left), |_| {
								Message::OutputChangeChannels(self.id, self.output.left(channel))
							})
							.size(15),
							radio(channel, Some(self.output.right), |_| {
								Message::OutputChangeChannels(self.id, self.output.right(channel))
							})
							.size(15)
						]
						.align_x(Center)
						.spacing(5)
						.into()
					}))
					.spacing(5)
				)
				.direction(scrollable::Direction::Horizontal(
					scrollable::Scrollbar::hidden(),
				))
				.width(Fit.max(328))
			]
			.spacing(5),
		)
		.padding(5)
		.style(container_with_radius(weaker_bordered_box, 5))
		.into()
	}

	pub fn midi_output_context_menu(&self) -> Element<'_, Message> {
		container(
			scrollable(
				row((0..16).map(|channel| {
					column![
						container(value(channel + 1).size(13).line_height(1.0)).padding(1),
						checkbox(self.output.midi & (1 << channel) != 0)
							.on_toggle(move |_| {
								Message::OutputChangeChannels(
									self.id,
									self.output.midi(self.output.midi ^ (1 << channel)),
								)
							})
							.size(15)
					]
					.align_x(Center)
					.spacing(5)
					.into()
				}))
				.spacing(5),
			)
			.direction(scrollable::Direction::Horizontal(
				scrollable::Scrollbar::hidden(),
			)),
		)
		.padding(5)
		.style(container_with_radius(weaker_bordered_box, 5))
		.into()
	}
}
