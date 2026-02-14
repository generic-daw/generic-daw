use crate::{
	arrangement_view::{Message, plugin::Plugin},
	components::icon_button,
	icons::{arrow_left_right, radius},
};
use generic_daw_core::{NodeId, PanMode};
use generic_daw_widget::{knob::Knob, peak_meter};
use iced::{
	Element, Fill,
	widget::{button, container, row},
};
use std::{cmp::Ordering, time::Instant};
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
	pub plugins: Vec<Plugin>,
	pub volume: f32,
	pub pan: PanMode,
	pub enabled: bool,
	pub bypassed: bool,
	pub peaks: NoDebug<[peak_meter::State; 2]>,
}

impl Node {
	pub fn new(ty: NodeType, id: NodeId) -> Self {
		Self {
			ty,
			id,
			plugins: Vec::new(),
			volume: 1.0,
			pan: PanMode::Balance(0.0),
			enabled: true,
			bypassed: false,
			peaks: [peak_meter::State::default(), peak_meter::State::default()].into(),
		}
	}

	pub fn update(&mut self, peaks: [f32; 2], now: Instant) {
		self.peaks[0].update(peaks[0], now);
		self.peaks[1].update(peaks[1], now);
	}

	pub fn pan_knob(&self, radius: f32) -> Element<'_, Message> {
		const RADIUS: f32 = 0.571_595_13; // 1.95 - sqrt(1.9)
		const SPACING: f32 = -0.286_380_5; // 2 * (2 * sqrt(1.9) - 2.9)

		match self.pan {
			PanMode::Balance(pan) => Knob::new(-1.0..=1.0, pan, |pan| {
				Message::ChannelPanChanged(self.id, PanMode::Balance(pan))
			})
			.center(0.0)
			.default(0.0)
			.radius(radius)
			.enabled(self.enabled)
			.tooltip(format_pan(pan))
			.into(),
			PanMode::Stereo(l, r) => row![
				container(
					Knob::new(-1.0..=1.0, l, move |l| {
						Message::ChannelPanChanged(self.id, PanMode::Stereo(l, r))
					})
					.center(0.0)
					.default(-1.0)
					.radius(radius * RADIUS)
					.enabled(self.enabled)
					.tooltip(format_pan(l))
				)
				.align_top(Fill),
				container(
					Knob::new(-1.0..=1.0, r, move |r| {
						Message::ChannelPanChanged(self.id, PanMode::Stereo(l, r))
					})
					.center(0.0)
					.default(1.0)
					.radius(radius * RADIUS)
					.enabled(self.enabled)
					.tooltip(format_pan(r))
				)
				.align_bottom(Fill)
			]
			.spacing(radius * SPACING)
			.width(2.0 * radius)
			.height(1.8 * radius)
			.into(),
		}
	}

	pub fn pan_switcher(&self) -> Element<'_, Message> {
		match self.pan {
			PanMode::Balance(..) => icon_button(
				arrow_left_right(),
				if self.enabled {
					button::primary
				} else {
					button::secondary
				},
			)
			.on_press(Message::ChannelPanChanged(
				self.id,
				PanMode::Stereo(-1.0, 1.0),
			))
			.into(),
			PanMode::Stereo(..) => icon_button(
				radius(),
				if self.enabled {
					button::primary
				} else {
					button::secondary
				},
			)
			.on_press(Message::ChannelPanChanged(self.id, PanMode::Balance(0.0)))
			.into(),
		}
	}
}

fn format_pan(pan: f32) -> String {
	let pan = (pan * 100.0) as i8;
	match pan.cmp(&0) {
		Ordering::Greater => format!("{}% right", pan.abs()),
		Ordering::Equal => "center".to_owned(),
		Ordering::Less => format!("{}% left", pan.abs()),
	}
}
