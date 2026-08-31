use crate::{
	arrangement_view::{AudioClipRef, AudioRecording, MidiClipRef, MidiRecording, format_db},
	state::Grid,
	widget::{
		ALPHA_1_3, ALPHA_2_3, LINE_HEIGHT, frames_per_px,
		playlist::{self, Action, Status},
		px_to_time, time_to_px,
	},
};
use generic_daw_core::{Transition, Transport, time::BeatTime};
use iced::{
	Event, Fill, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Layout, Renderer as _, Shell, Text, Widget,
		graphics::{
			geometry::Renderer as _,
			mesh::{self, Renderer as _},
			text::Paragraph,
		},
		layout::{Limits, Node},
		mouse::{self, Click, Cursor, Interaction, click::Kind},
		renderer::{Quad, Style},
		text::{
			Alignment, Ellipsis, LineHeight, Renderer as _, Shaping, Wrapping, paragraph::Plain,
		},
		widget::{Tree, tree},
	},
	alignment::Vertical,
	border, debug, padding,
	widget::canvas::{self, Frame, Path, Stroke, path::Builder},
	window,
};
use std::{
	borrow::{Borrow, BorrowMut},
	cell::RefCell,
	sync::Arc,
};

#[derive(Default, PartialEq)]
struct ClipInfo {
	offset: BeatTime,
	stretch: f32,
	volume: f32,
	fade_start: Transition,
	fade_end: Transition,
	addr: usize,
}

struct State {
	mesh_cache: RefCell<mesh::Cache>,
	canvas_cache: RefCell<canvas::Cache>,
	volume_text: Plain<Paragraph>,
	last_click: Option<Click>,
	last_bounds: Rectangle,
	last_info: ClipInfo,
	last_theme: RefCell<Option<Theme>>,
	show_controls: bool,
	selected: bool,
	enabled: bool,
}

impl Default for State {
	fn default() -> Self {
		Self {
			mesh_cache: RefCell::new(mesh::Cache::new(Arc::default())),
			canvas_cache: RefCell::default(),
			volume_text: Plain::default(),
			last_click: None,
			last_bounds: Rectangle::default(),
			last_info: ClipInfo::default(),
			last_theme: RefCell::default(),
			show_controls: false,
			selected: false,
			enabled: true,
		}
	}
}

#[derive(Clone, Debug)]
pub enum Inner<'a> {
	AudioClip(AudioClipRef<'a>),
	MidiClip(MidiClipRef<'a>),
	AudioRecording(&'a AudioRecording),
	MidiRecording(&'a MidiRecording),
}

impl<'a> From<AudioClipRef<'a>> for Inner<'a> {
	fn from(value: AudioClipRef<'a>) -> Self {
		Self::AudioClip(value)
	}
}

impl<'a> From<MidiClipRef<'a>> for Inner<'a> {
	fn from(value: MidiClipRef<'a>) -> Self {
		Self::MidiClip(value)
	}
}

impl<'a> From<&'a AudioRecording> for Inner<'a> {
	fn from(value: &'a AudioRecording) -> Self {
		Self::AudioRecording(value)
	}
}

impl<'a> From<&'a MidiRecording> for Inner<'a> {
	fn from(value: &'a MidiRecording) -> Self {
		Self::MidiRecording(value)
	}
}

#[derive(Clone, Debug)]
pub struct Clip<'a, Message> {
	pub(super) inner: Inner<'a>,
	playlist: &'a RefCell<playlist::State>,
	transport: &'a Transport,
	grid: &'a Grid,
	enabled: bool,
	f: fn(Action) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for Clip<'_, Message> {
	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State::default())
	}

	fn diff(&mut self, tree: &mut Tree) {
		let state = tree.state.downcast_mut::<State>();

		let playlist = self.playlist.borrow();

		let info = ClipInfo {
			offset: match self.inner {
				Inner::AudioClip(inner) => {
					inner.clip.position.offset().to_beat_time(self.transport)
				}
				Inner::MidiClip(inner) => inner.clip.position.offset(),
				Inner::AudioRecording(..) | Inner::MidiRecording(..) => BeatTime::ZERO,
			},
			stretch: match self.inner {
				Inner::AudioClip(inner) => {
					frames_per_px(playlist.scale, self.transport) * inner.clip.stretch as f32
				}
				Inner::MidiClip(..) | Inner::MidiRecording(..) => 1.0,
				Inner::AudioRecording(..) => frames_per_px(playlist.scale, self.transport),
			},
			volume: match self.inner {
				Inner::AudioClip(inner) => inner.clip.volume,
				Inner::MidiClip(..) | Inner::AudioRecording(..) | Inner::MidiRecording(..) => 1.0,
			},
			fade_start: match self.inner {
				Inner::AudioClip(inner) => inner.clip.fade_start,
				Inner::MidiClip(..) | Inner::AudioRecording(..) | Inner::MidiRecording(..) => {
					Transition::default()
				}
			},
			fade_end: match self.inner {
				Inner::AudioClip(inner) => inner.clip.fade_end,
				Inner::MidiClip(..) | Inner::AudioRecording(..) | Inner::MidiRecording(..) => {
					Transition::default()
				}
			},
			addr: match self.inner {
				Inner::AudioClip(inner) => std::ptr::from_ref(inner.sample).addr(),
				Inner::MidiClip(inner) => std::ptr::from_ref(inner.pattern).addr(),
				Inner::AudioRecording(inner) => std::ptr::from_ref(inner).addr(),
				Inner::MidiRecording(inner) => std::ptr::from_ref(inner).addr(),
			},
		};

		if state.last_info != info {
			state.last_info = info;
			state.canvas_cache.get_mut().clear();
			if !state.mesh_cache.get_mut().is_empty() {
				state.mesh_cache.get_mut().update(Arc::default());
			}
		}

		if state.enabled != self.enabled {
			state.enabled = self.enabled;
			state.canvas_cache.get_mut().clear();
		}
	}

	fn size(&self) -> Size<Length> {
		Size::new(Fill, Fill)
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		if let Inner::AudioClip(inner) = self.inner {
			let content = format_db(inner.clip.volume);
			tree.state.downcast_mut::<State>().volume_text.update(Text {
				content: &*content,
				bounds: Size::INFINITE,
				size: renderer.default_size(),
				line_height: LineHeight::Relative(1.0),
				font: renderer.default_font(),
				align_x: Alignment::Center,
				align_y: Vertical::Bottom,
				shaping: Shaping::Auto,
				wrapping: Wrapping::None,
				ellipsis: Ellipsis::None,
				hint_factor: renderer.hint_factor(),
			});
		}

		let playlist = self.playlist.borrow();

		let (start, end) = match self.inner {
			Inner::AudioClip(inner) => (
				inner.clip.position.start(),
				inner.clip.position.end(self.transport),
			),
			Inner::MidiClip(inner) => (inner.clip.position.start(), inner.clip.position.end()),
			Inner::AudioRecording(inner) => (inner.position, inner.end(self.transport)),
			Inner::MidiRecording(inner) => (inner.position, inner.end(self.transport)),
		};

		let start = time_to_px(start, playlist.position, playlist.scale, self.transport);
		let end = time_to_px(end, playlist.position, playlist.scale, self.transport);

		Node::new(Size::new(end - start, limits.max().height)).translate(Vector::new(start, 0.0))
	}

	fn update(
		&mut self,
		tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		cursor: Cursor,
		_renderer: &Renderer,
		shell: &mut Shell<'_, Message>,
		viewport: &Rectangle,
	) {
		let state = tree.state.downcast_mut::<State>();

		if let Event::Window(window::Event::RedrawRequested(..)) = event
			&& let Some(bounds) = layout.bounds().intersection(viewport)
		{
			let bounds = bounds - Vector::new(layout.position().x, layout.position().y);

			if state.last_bounds != bounds {
				state.last_bounds = bounds;
				state.canvas_cache.get_mut().clear();
				if !state.mesh_cache.get_mut().is_empty() {
					state.mesh_cache.get_mut().update(Arc::default());
				}
			}
		}

		let (Inner::AudioClip(AudioClipRef { index, .. })
		| Inner::MidiClip(MidiClipRef { index, .. })) = self.inner
		else {
			return;
		};

		let playlist = &mut *self.playlist.borrow_mut();

		if let Event::Window(window::Event::RedrawRequested(..)) = event {
			let selected = playlist.primary.contains(&index) || playlist.secondary.contains(&index);
			if state.selected != selected {
				state.selected = selected;
				state.canvas_cache.get_mut().clear();
			}
		}

		if shell.is_event_captured() {
			return;
		}

		if let Inner::AudioClip(..) = self.inner {
			let show_controls = match playlist.status {
				Status::None => {
					cursor.is_over(layout.bounds().intersection(viewport).unwrap_or_default())
				}
				Status::DraggingVolume(..)
				| Status::FadingStartLen(..)
				| Status::FadingStartP(..)
				| Status::FadingEndLen(..)
				| Status::FadingEndP(..) => state.selected,
				_ => false,
			};

			if state.show_controls != show_controls {
				state.show_controls = show_controls;
				state.canvas_cache.get_mut().clear();

				if !matches!(event, Event::Window(window::Event::RedrawRequested(..))) {
					shell.request_redraw();
				}
			}
		}

		let Some(cursor) = cursor.position_in(*viewport) else {
			return;
		};

		let clip_bounds = layout.bounds() - Vector::new(viewport.x, viewport.y);
		if !clip_bounds.contains(cursor) {
			return;
		}

		let header_height = header_height(&layout);

		match event {
			Event::Mouse(mouse::Event::ButtonPressed {
				button: mouse::Button::Left,
				modifiers,
			}) if playlist.status == Status::None => {
				let mut clear = playlist.primary.insert(index);

				let new_click = Click::new(cursor, mouse::Button::Left, state.last_click);
				state.last_click = Some(new_click);

				let time = px_to_time(cursor.x, playlist.position, playlist.scale, self.transport);

				match self.inner {
					Inner::AudioClip(inner) => 'block: {
						if cursor.y - clip_bounds.y < header_height {
							break 'block;
						}

						let frames_per_px = frames_per_px(playlist.scale, self.transport);
						let fade_start_px = inner.clip.fade_start.len.to_frames(self.transport)
							as f32 / frames_per_px;
						let fade_end_px = inner.clip.fade_end.len.to_frames(self.transport) as f32
							/ -frames_per_px;

						let fade_start_control = Point::new(
							clip_bounds.x + inner.clip.fade_start.p.x * fade_start_px,
							clip_bounds.y
								+ header_height + (1.0 - inner.clip.fade_start.p.y)
								* (clip_bounds.height - header_height),
						);

						let fade_end_control = Point::new(
							clip_bounds.x
								+ clip_bounds.width + inner.clip.fade_end.p.x * fade_end_px,
							clip_bounds.y
								+ header_height + (1.0 - inner.clip.fade_end.p.y)
								* (clip_bounds.height - header_height),
						);

						let fade_start_control_dist = cursor.distance(fade_start_control);
						let fade_end_control_dist = cursor.distance(fade_end_control);

						match (
							fade_start_px >= 8.0 && fade_start_control_dist <= 5.0,
							fade_end_px <= -8.0 && fade_end_control_dist <= 5.0,
							fade_start_control_dist <= fade_end_control_dist,
						) {
							(true, true, true) | (true, false, _) => {
								if new_click.kind() == Kind::Double {
									shell.publish((self.f)(Action::FadeStartToggleSymmetric));
								}
								playlist.status =
									Status::FadingStartP(inner.index.0, inner.index.1);
								shell.capture_event();
								return;
							}
							(true, true, false) | (false, true, _) => {
								if new_click.kind() == Kind::Double {
									shell.publish((self.f)(Action::FadeEndToggleSymmetric));
								}
								playlist.status = Status::FadingEndP(inner.index.0, inner.index.1);
								shell.capture_event();
								return;
							}
							(false, false, _) => {
								let bounds =
									layout.bounds().intersection(viewport).unwrap_or_default();
								let volume_control = Point::new(
									bounds.x + bounds.width / 2.0 - viewport.x,
									clip_bounds.y + clip_bounds.height,
								);
								if bounds.width >= 8.0 && cursor.distance(volume_control) <= 10.0 {
									if new_click.kind() == Kind::Double {
										shell.publish((self.f)(Action::InvertPolarity));
									}
									playlist.status = Status::DraggingVolume(cursor.y);
									shell.capture_event();
									return;
								}
							}
						}

						if cursor.y - clip_bounds.y > header_height + 12.0 {
							break 'block;
						}

						let fade_start_tab_dist =
							(clip_bounds.x + fade_start_px + 4.0 - cursor.x).abs();
						let fade_end_tab_dist = (clip_bounds.x + clip_bounds.width + fade_end_px
							- 4.0 - cursor.x)
							.abs();

						let left_of_start_tab = clip_bounds.x + fade_start_px > cursor.x;
						let left_of_end_tab =
							clip_bounds.x + clip_bounds.width + fade_end_px > cursor.x;

						let use_start = match (fade_start_tab_dist <= 6.0, fade_end_tab_dist <= 6.0)
						{
							(true, false) => left_of_end_tab,
							(false, true) => left_of_start_tab,
							(true, true) => {
								if fade_start_tab_dist <= fade_end_tab_dist {
									left_of_end_tab
								} else {
									left_of_start_tab
								}
							}
							(false, false) => break 'block,
						};

						playlist.status = if use_start {
							Status::FadingStartLen(time)
						} else {
							Status::FadingEndLen(time)
						};
						shell.capture_event();
						return;
					}
					Inner::MidiClip(..) => {
						if new_click.kind() == Kind::Double {
							shell.publish((self.f)(Action::Open(index.0, index.1)));
							shell.capture_event();
							return;
						}
					}
					Inner::AudioRecording(..) | Inner::MidiRecording(..) => unreachable!(),
				}

				let start_pixel = clip_bounds.x;
				let end_pixel = clip_bounds.x + clip_bounds.width;
				let start_offset = cursor.x - start_pixel;
				let end_offset = end_pixel - cursor.x;
				let border = 10f32.min(clip_bounds.width / 3.0);

				playlist.status = match (
					modifiers.command(),
					modifiers.shift(),
					start_offset < border,
					end_offset < border,
					cursor.y - clip_bounds.y.max(0.0) < header_height,
				) {
					(false, false, false, false, _) => Status::Dragging(index.0, time),
					(false, _, true, false, _) => Status::TrimmingStart(time),
					(false, _, false, true, _) => Status::TrimmingEnd(time),
					(true, false, _, _, _) => {
						clear = false;
						let time = self.grid.maybe_snap(time, *modifiers, |time| {
							time.round(self.grid.beats_snap_step(playlist.scale, self.transport))
						});
						Status::Selecting(index.0, index.0, time, time)
					}
					(false, true, _, _, _) => {
						shell.publish((self.f)(Action::Clone));
						Status::Dragging(index.0, time)
					}
					(true, true, _, _, false) => Status::DraggingSlip(time),
					(true, true, _, _, true) => {
						let time = self.grid.maybe_snap(time, *modifiers, |time| {
							time.round(self.grid.beats_snap_step(playlist.scale, self.transport))
						});
						shell.publish((self.f)(Action::SplitAt(time)));
						Status::DraggingSplit(time)
					}
					(_, _, true, true, _) => unreachable!(),
				};

				shell.capture_event();
				shell.request_redraw();

				if clear {
					playlist.primary.clear();
					playlist.primary.insert(index);
				}
			}
			Event::Mouse(mouse::Event::ButtonPressed {
				button: mouse::Button::Right,
				..
			}) if playlist.status == Status::None => {
				playlist.primary.clear();
				playlist.primary.insert(index);
				playlist.status = Status::Deleting;
				shell.publish((self.f)(Action::Delete));
				shell.capture_event();
			}
			Event::Mouse(mouse::Event::CursorMoved { .. })
				if playlist.status == Status::Deleting =>
			{
				playlist.primary.insert(index);
			}
			_ => {}
		}
	}

	fn draw(
		&self,
		tree: &Tree,
		renderer: &mut Renderer,
		theme: &Theme,
		_style: &Style,
		layout: Layout<'_>,
		_cursor: Cursor,
		viewport: &Rectangle,
	) {
		let Some(bounds) = layout.bounds().intersection(viewport) else {
			return;
		};

		let state = tree.state.downcast_ref::<State>();

		let header_height = header_height(&layout);

		let mut upper_bounds = bounds;
		upper_bounds.height = upper_bounds.height.min(header_height);

		let color = match &self.inner {
			Inner::AudioClip(..) | Inner::MidiClip(..) => match (state.enabled, state.selected) {
				(true, true) => theme.palette().danger.weak.color,
				(true, false) => theme.palette().primary.weak.color,
				(false, true) => theme.palette().secondary.strong.color,
				(false, false) => theme.palette().secondary.weak.color,
			},
			Inner::AudioRecording(..) | Inner::MidiRecording(..) => {
				theme.palette().warning.weak.color
			}
		};

		renderer.fill_quad(
			Quad {
				bounds: upper_bounds,
				..Quad::default()
			},
			color,
		);

		if upper_bounds.width > 6.0 {
			let clip_name = match self.inner {
				Inner::AudioClip(inner) => &*inner.sample.name,
				Inner::MidiClip(inner) => &*inner.pattern.name,
				Inner::AudioRecording(inner) => &*inner.name,
				Inner::MidiRecording(inner) => &*inner.name,
			};

			let clip_name = Text {
				content: clip_name.into(),
				bounds: upper_bounds.shrink(padding::horizontal(3)).size(),
				size: renderer.default_size(),
				line_height: LineHeight::default(),
				font: renderer.default_font(),
				align_x: Alignment::Left,
				align_y: Vertical::Center,
				shaping: Shaping::Auto,
				wrapping: Wrapping::None,
				ellipsis: Ellipsis::Middle,
				hint_factor: renderer.hint_factor(),
			};

			renderer.fill_text(
				clip_name,
				upper_bounds.position()
					+ Vector::new(
						3.0,
						if upper_bounds.y == viewport.y {
							upper_bounds.height - header_height / 2.0
						} else {
							header_height / 2.0
						},
					),
				theme.palette().background.strong.text,
				upper_bounds,
			);
		}

		let lower_bounds = bounds.shrink(padding::top(upper_bounds.height));
		if lower_bounds.height <= 0.0 {
			return;
		}

		renderer.fill_quad(
			Quad {
				bounds: lower_bounds,
				border: border::width(1).color(color),
				..Quad::default()
			},
			color.scale_alpha(ALPHA_1_3),
		);

		let mesh_cache = &mut *state.mesh_cache.borrow_mut();
		let canvas_cache = &mut *state.canvas_cache.borrow_mut();
		let last_theme = &mut *state.last_theme.borrow_mut();

		if last_theme.as_ref() != Some(theme) {
			*last_theme = Some(theme.clone());
			canvas_cache.clear();
			if !mesh_cache.is_empty() {
				mesh_cache.update(Arc::default());
			}
		}

		let playlist = self.playlist.borrow();
		let frames_per_px = frames_per_px(playlist.scale, self.transport);
		let unclipped_bounds = layout.bounds().shrink(padding::top(header_height));

		match self.inner {
			Inner::AudioClip(inner) => {
				if mesh_cache.is_empty()
					&& let Some(mesh) = debug::time_with("Waveform", || {
						let resample_ratio = inner.sample.resample_ratio(self.transport);
						inner.sample.lods.mesh(
							&inner.sample.samples,
							inner.clip.position.offset() / resample_ratio,
							self.transport,
							inner.clip.volume,
							Transition {
								len: inner.clip.fade_start.len / resample_ratio,
								..inner.clip.fade_start
							},
							Transition {
								len: inner.clip.fade_end.len / resample_ratio,
								..inner.clip.fade_end
							},
							frames_per_px / resample_ratio as f32 * inner.clip.stretch as f32,
							theme.palette().background.strong.text,
							unclipped_bounds,
							lower_bounds,
						)
					}) {
					mesh_cache.update(Arc::from([mesh]));
				}

				let fill_canvas = |renderer: &Renderer, frame: &mut Frame| {
					let start_offset = Vector::new(
						unclipped_bounds.x - lower_bounds.x,
						unclipped_bounds.y - lower_bounds.y,
					);
					let end_offset = start_offset + Vector::new(layout.bounds().width, 0.0);

					let fade_start_px =
						inner.clip.fade_start.len.to_frames(self.transport) as f32 / frames_per_px;
					let fade_end_px =
						inner.clip.fade_end.len.to_frames(self.transport) as f32 / -frames_per_px;

					let fade = |b: &mut Builder, fade: Transition, fade_px: f32, offset: Vector| {
						b.move_to(Point::new(0.0, unclipped_bounds.height) + offset);
						if fade.symmetric {
							b.quadratic_curve_to(
								Point::new(
									(0.5 * fade.p.x) * fade_px,
									(1.0 - 0.5 * fade.p.y) * unclipped_bounds.height,
								) + offset,
								Point::new(0.5 * fade_px, 0.5 * unclipped_bounds.height) + offset,
							);
							b.quadratic_curve_to(
								Point::new(
									(1.0 - 0.5 * fade.p.x) * fade_px,
									(0.5 * fade.p.y) * unclipped_bounds.height,
								) + offset,
								Point::new(fade_px, 0.0) + offset,
							);
						} else {
							b.quadratic_curve_to(
								Point::new(
									fade.p.x * fade_px,
									(1.0 - fade.p.y) * unclipped_bounds.height,
								) + offset,
								Point::new(fade_px, 0.0) + offset,
							);
						}
					};

					if fade_start_px >= 0.5 {
						frame.stroke(
							&Path::new(|b| {
								fade(b, inner.clip.fade_start, fade_start_px, start_offset);
							}),
							Stroke::default().with_color(color).with_width(2.0),
						);

						frame.fill(
							&Path::new(|b| {
								fade(b, inner.clip.fade_start, fade_start_px, start_offset);
								b.line_to(Point::ORIGIN + start_offset);
								b.close();
							}),
							color.scale_alpha(ALPHA_1_3),
						);
					}

					if fade_end_px <= -0.5 {
						frame.stroke(
							&Path::new(|b| {
								fade(b, inner.clip.fade_end, fade_end_px, end_offset);
							}),
							Stroke::default().with_color(color).with_width(2.0),
						);

						frame.fill(
							&Path::new(|b| {
								fade(b, inner.clip.fade_end, fade_end_px, end_offset);
								b.line_to(Point::ORIGIN + end_offset);
								b.close();
							}),
							color.scale_alpha(ALPHA_1_3),
						);
					}

					let lower_edge = unclipped_bounds.y + unclipped_bounds.height - lower_bounds.y;

					if state.show_controls {
						frame.fill(
							&Path::new(|b| {
								b.move_to(Point::new(fade_start_px, 0.0) + start_offset);
								b.line_to(Point::new(fade_start_px + 8.0, 0.0) + start_offset);
								b.line_to(Point::new(fade_start_px, 12.0) + start_offset);
								b.close();
							}),
							color,
						);

						frame.fill(
							&Path::new(|b| {
								b.move_to(Point::new(fade_end_px, 0.0) + end_offset);
								b.line_to(Point::new(fade_end_px - 8.0, 0.0) + end_offset);
								b.line_to(Point::new(fade_end_px, 12.0) + end_offset);
								b.close();
							}),
							color,
						);

						if lower_bounds.width >= 8.0 {
							let control = Point::new(lower_bounds.width / 2.0, lower_edge);

							frame.fill(
								&Path::circle(control, 4.0),
								theme.palette().background.strong.text,
							);

							frame.fill(&Path::circle(control, 2.5), color);
						}

						if fade_start_px >= 8.0 {
							let control = Point::new(
								inner.clip.fade_start.p.x * fade_start_px,
								(1.0 - inner.clip.fade_start.p.y) * unclipped_bounds.height,
							) + start_offset;

							frame.fill(
								&Path::circle(control, 4.0),
								theme.palette().background.strong.text,
							);

							frame.fill(&Path::circle(control, 2.5), color);
						}

						if fade_end_px <= -8.0 {
							let control = Point::new(
								inner.clip.fade_end.p.x * fade_end_px,
								(1.0 - inner.clip.fade_end.p.y) * unclipped_bounds.height,
							) + end_offset;

							frame.fill(
								&Path::circle(control, 4.0),
								theme.palette().background.strong.text,
							);

							frame.fill(&Path::circle(control, 2.5), color);
						}
					}

					if state.show_controls || state.volume_text.content() != "0.0 dB" {
						let size = state.volume_text.min_bounds().expand((4.0, 4.0));

						if lower_bounds.width >= size.width && lower_edge >= size.height + 5.0 {
							let control = Point::new(lower_bounds.width / 2.0, lower_edge - 6.0);

							frame.fill(
								&Path::rounded_rectangle(
									control - Vector::new(size.width / 2.0, size.height - 2.0),
									size,
									2.into(),
								),
								color.scale_alpha(ALPHA_2_3),
							);

							frame.fill_text(canvas::Text {
								content: state.volume_text.content().to_owned(),
								position: control,
								max_width: f32::INFINITY,
								color: theme.palette().background.strong.text,
								size: renderer.default_size(),
								line_height: LineHeight::Relative(1.0),
								font: renderer.default_font(),
								align_x: Alignment::Center,
								align_y: Vertical::Bottom,
								shaping: Shaping::Auto,
								wrapping: Wrapping::None,
								ellipsis: Ellipsis::None,
							});
						}
					}
				};

				renderer.with_translation(
					Vector::new(lower_bounds.x, lower_bounds.y),
					|renderer| {
						renderer.draw_mesh_cache(mesh_cache.clone());
						renderer.draw_geometry(canvas_cache.draw(
							renderer,
							lower_bounds.size(),
							|frame| {
								debug::time_with("Clip Overlay", || fill_canvas(renderer, frame));
							},
						));
					},
				);
			}
			Inner::MidiClip(inner) => 'block: {
				if lower_bounds.width < 1.0 || inner.pattern.notes.is_empty() {
					break 'block;
				}

				let (min, max) = inner
					.pattern
					.notes
					.iter()
					.map(|note| note.key.0)
					.fold((255, 0), |(min, max), key| (key.min(min), key.max(max)));

				let note_height = unclipped_bounds.height / f32::from(max - min + 3);
				let offset = Vector::new(layout.position().x, layout.position().y + header_height);

				for note in &inner.pattern.notes {
					let start_pixel = note
						.position
						.start()
						.saturating_sub(inner.clip.position.offset())
						.to_frames(self.transport) as f32
						/ frames_per_px;
					let end_pixel = note
						.position
						.end()
						.saturating_sub(inner.clip.position.offset())
						.to_frames(self.transport) as f32
						/ frames_per_px;

					let top_pixel = f32::from(max - note.key.0 + 1) * note_height;

					let Some(bounds) = Rectangle::new(
						Point::new(start_pixel, top_pixel) + offset,
						Size::new(end_pixel - start_pixel, note_height),
					)
					.intersection(&lower_bounds) else {
						continue;
					};

					renderer.fill_quad(
						Quad {
							bounds,
							..Quad::default()
						},
						theme.palette().background.strong.text,
					);
				}
			}
			Inner::AudioRecording(inner) => {
				if mesh_cache.is_empty()
					&& let Some(mesh) = debug::time_with("Waveform", || {
						inner.lods.mesh(
							&inner.samples,
							self.transport,
							frames_per_px,
							theme.palette().background.strong.text,
							unclipped_bounds,
							lower_bounds,
						)
					}) {
					mesh_cache.update(Arc::from([mesh]));
				}

				renderer.with_translation(
					Vector::new(lower_bounds.x, lower_bounds.y),
					|renderer| {
						renderer.draw_mesh_cache(mesh_cache.clone());
					},
				);
			}
			Inner::MidiRecording(inner) => 'block: {
				if lower_bounds.width < 1.0 || (inner.notes.is_empty() && inner.playing.is_empty())
				{
					break 'block;
				}

				let (min, max) = inner
					.notes
					.iter()
					.map(|note| note.key.0)
					.chain(inner.playing.keys().map(|(_, key)| key.as_int()))
					.fold((255, 0), |(min, max), key| (key.min(min), key.max(max)));

				let note_height = unclipped_bounds.height / f32::from(max - min + 3);
				let offset = Vector::new(layout.position().x, layout.position().y + header_height);

				for note in &inner.notes {
					let start_pixel =
						note.position.start().to_frames(self.transport) as f32 / frames_per_px;
					let end_pixel =
						note.position.end().to_frames(self.transport) as f32 / frames_per_px;

					let top_pixel = f32::from(max - note.key.0 + 1) * note_height;

					let Some(bounds) = Rectangle::new(
						Point::new(start_pixel, top_pixel) + offset,
						Size::new(end_pixel - start_pixel, note_height),
					)
					.intersection(&lower_bounds) else {
						continue;
					};

					renderer.fill_quad(
						Quad {
							bounds,
							..Quad::default()
						},
						theme.palette().background.strong.text,
					);
				}

				for ((_, key), (_, start)) in &inner.playing {
					let start_pixel = start.to_frames(self.transport) as f32 / frames_per_px;
					let end_pixel =
						inner.end(self.transport).to_frames(self.transport) as f32 / frames_per_px;

					let top_pixel = f32::from(max - key.as_int() + 1) * note_height;

					let Some(bounds) = Rectangle::new(
						Point::new(start_pixel, top_pixel) + offset,
						Size::new(end_pixel - start_pixel, note_height),
					)
					.intersection(&lower_bounds) else {
						continue;
					};

					renderer.fill_quad(
						Quad {
							bounds,
							..Quad::default()
						},
						theme.palette().background.strong.text,
					);
				}
			}
		}
	}

	fn mouse_interaction(
		&self,
		_tree: &Tree,
		layout: Layout<'_>,
		cursor: Cursor,
		viewport: &Rectangle,
		_renderer: &Renderer,
	) -> Interaction {
		if !cursor.is_over(*viewport) {
			return Interaction::default();
		}

		let Some(cursor) = cursor.position_in(layout.bounds()) else {
			return Interaction::default();
		};

		let header_height = header_height(&layout);
		let playlist = self.playlist.borrow();

		match self.inner {
			Inner::AudioClip(inner) => 'block: {
				if cursor.y + layout.position().y - viewport.y < header_height {
					break 'block;
				}

				let frames_per_px = frames_per_px(playlist.scale, self.transport);
				let fade_start_px =
					inner.clip.fade_start.len.to_frames(self.transport) as f32 / frames_per_px;
				let fade_end_px =
					inner.clip.fade_end.len.to_frames(self.transport) as f32 / -frames_per_px;

				let fade_start_control = Point::new(
					inner.clip.fade_start.p.x * fade_start_px,
					(1.0 - inner.clip.fade_start.p.y) * (layout.bounds().height - header_height)
						+ header_height,
				);

				let fade_end_control = Point::new(
					layout.bounds().width + inner.clip.fade_end.p.x * fade_end_px,
					(1.0 - inner.clip.fade_end.p.y) * (layout.bounds().height - header_height)
						+ header_height,
				);

				if fade_start_px >= 8.0 && cursor.distance(fade_start_control) <= 5.0
					|| fade_end_px <= -8.0 && cursor.distance(fade_end_control) <= 5.0
				{
					return Interaction::Crosshair;
				}

				let bounds = layout.bounds().intersection(viewport).unwrap_or_default();
				let volume_control = Point::new(
					bounds.x + bounds.width / 2.0 - layout.position().x,
					layout.bounds().height,
				);
				if bounds.width >= 8.0 && cursor.distance(volume_control) <= 10.0 {
					return Interaction::ResizingVertically;
				}

				if cursor.y > header_height + 12.0 {
					break 'block;
				}

				if (fade_start_px + 4.0 - cursor.x).abs() <= 6.0
					|| (layout.bounds().width + fade_end_px - 4.0 - cursor.x).abs() <= 6.0
				{
					return Interaction::Pointer;
				}
			}
			Inner::MidiClip(..) | Inner::AudioRecording(..) | Inner::MidiRecording(..) => {}
		}

		match self.inner {
			Inner::AudioClip(..) | Inner::MidiClip(..) => {
				let border = 10f32.min(layout.bounds().width / 3.0);
				match (cursor.x < border, layout.bounds().width - cursor.x < border) {
					(false, false) => Interaction::Grab,
					(true, false) | (false, true) => Interaction::ResizingHorizontally,
					(true, true) => unreachable!(),
				}
			}
			Inner::AudioRecording(..) | Inner::MidiRecording(..) => Interaction::NotAllowed,
		}
	}
}

impl<'a, Message> Clip<'a, Message> {
	pub fn new(
		inner: impl Into<Inner<'a>>,
		playlist: &'a RefCell<playlist::State>,
		transport: &'a Transport,
		grid: &'a Grid,
		enabled: bool,
		f: fn(Action) -> Message,
	) -> Self {
		Self {
			inner: inner.into(),
			playlist,
			transport,
			grid,
			enabled,
			f,
		}
	}
}

impl<'a, Message: 'a> Borrow<dyn Widget<Message, Theme, Renderer> + 'a> for Clip<'a, Message> {
	fn borrow(&self) -> &(dyn Widget<Message, Theme, Renderer> + 'a) {
		self
	}
}

impl<'a, Message: 'a> BorrowMut<dyn Widget<Message, Theme, Renderer> + 'a> for Clip<'a, Message> {
	fn borrow_mut(&mut self) -> &mut (dyn Widget<Message, Theme, Renderer> + 'a) {
		self
	}
}

fn header_height(layout: &Layout<'_>) -> f32 {
	if layout.bounds().height < 2.0 * LINE_HEIGHT {
		0.0
	} else {
		LINE_HEIGHT
	}
}
