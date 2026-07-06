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
use generic_daw_core::{NodeId, PanMode, Utility};
use generic_daw_widget::{context_menu::ContextMenu, knob::Knob, peak_meter};
use iced::{
	Element, Fill, padding,
	widget::{self, column, container, row, rule},
};
use std::time::Instant;
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
	pub peaks: NoDebug<[peak_meter::State; 2]>,
	pub polyphony: usize,
}

impl Node {
	pub fn new(ty: NodeType, id: NodeId) -> Self {
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
}
