use crate::{
	arrangement_view::{AudioClipRef, MidiClipRef, Recording, format_db},
	widget::{
		ALPHA_1_3, ALPHA_2_3, LINE_HEIGHT, beats_snap_step, maybe_snap,
		playlist::{self, Action, Status},
		px_to_time, samples_per_px, time_to_px,
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
		text::{Paragraph as _, Renderer as _},
		widget::{Tree, tree},
	},
	alignment::Vertical,
	border, debug, padding,
	widget::{
		canvas::{self, Frame, Path, Stroke, path::Builder},
		text::{Alignment, Ellipsis, LineHeight, Shaping, Wrapping},
	},
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
	Recording(&'a Recording),
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

impl<'a> From<&'a Recording> for Inner<'a> {
	fn from(value: &'a Recording) -> Self {
		Self::Recording(value)
	}
}

#[derive(Clone, Debug)]
pub struct Clip<'a, Message> {
	pub(super) inner: Inner<'a>,
	playlist: &'a RefCell<playlist::State>,
	transport: &'a Transport,
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
				Inner::Recording(..) => BeatTime::ZERO,
			},
			stretch: match self.inner {
				Inner::AudioClip(inner) => {
					samples_per_px(playlist.scale, self.transport) * inner.clip.stretch as f32
				}
				Inner::MidiClip(..) => 1.0,
				Inner::Recording(..) => samples_per_px(playlist.scale, self.transport),
			},
			volume: match self.inner {
				Inner::AudioClip(inner) => inner.clip.volume,
				Inner::MidiClip(..) | Inner::Recording(..) => 1.0,
			},
			fade_start: match self.inner {
				Inner::AudioClip(inner) => inner.clip.fade_start,
				Inner::MidiClip(..) | Inner::Recording(..) => Transition::default(),
			},
			fade_end: match self.inner {
				Inner::AudioClip(inner) => inner.clip.fade_end,
				Inner::MidiClip(..) | Inner::Recording(..) => Transition::default(),
			},
			addr: match self.inner {
				Inner::AudioClip(inner) => std::ptr::from_ref(inner.sample).addr(),
				Inner::MidiClip(inner) => std::ptr::from_ref(inner.pattern).addr(),
				Inner::Recording(inner) => std::ptr::from_ref(inner).addr(),
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

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
		let playlist = self.playlist.borrow();

		let (start, end) = match self.inner {
			Inner::AudioClip(inner) => (
				inner.clip.position.start(),
				inner.clip.position.end(self.transport),
			),
			Inner::MidiClip(inner) => (inner.clip.position.start(), inner.clip.position.end()),
			Inner::Recording(inner) => (inner.position, inner.position + inner.len(self.transport)),
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

		match event {
			Event::Mouse(mouse::Event::ButtonPressed { button, modifiers })
				if playlist.status == Status::None =>
			{
				let mut clear = playlist.primary.insert(index);

				match button {
					mouse::Button::Left => {
						let new_click = Click::new(cursor, mouse::Button::Left, state.last_click);
						state.last_click = Some(new_click);

						let time =
							px_to_time(cursor.x, playlist.position, playlist.scale, self.transport);

						match self.inner {
							Inner::AudioClip(inner) => 'block: {
								if cursor.y - clip_bounds.y.max(0.0) < LINE_HEIGHT {
									break 'block;
								}

								let samples_per_px = samples_per_px(playlist.scale, self.transport);
								let fade_start_px =
									inner.clip.fade_start.len.to_samples(self.transport) as f32
										/ samples_per_px;
								let fade_end_px = inner.clip.fade_end.len.to_samples(self.transport)
									as f32 / -samples_per_px;

								let fade_start_control = Point::new(
									clip_bounds.x + inner.clip.fade_start.p.x * fade_start_px,
									clip_bounds.y
										+ LINE_HEIGHT + (1.0 - inner.clip.fade_start.p.y)
										* (layout.bounds().height - LINE_HEIGHT),
								);

								let fade_end_control = Point::new(
									clip_bounds.x
										+ layout.bounds().width + inner.clip.fade_end.p.x * fade_end_px,
									clip_bounds.y
										+ LINE_HEIGHT + (1.0 - inner.clip.fade_end.p.y)
										* (layout.bounds().height - LINE_HEIGHT),
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
											shell.publish((self.f)(
												Action::FadeStartToggleSymmetric,
											));
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
										playlist.status =
											Status::FadingEndP(inner.index.0, inner.index.1);
										shell.capture_event();
										return;
									}
									(false, false, _) => {
										let bounds =
											layout.bounds().intersection(viewport).unwrap()
												- Vector::new(viewport.x, viewport.y);
										let volume_control = bounds.position()
											+ Vector::new(bounds.width / 2.0, bounds.height);
										if bounds.width >= 8.0
											&& cursor.distance(volume_control) <= 10.0
										{
											if new_click.kind() == Kind::Double {
												shell.publish((self.f)(Action::InvertPolarity));
											}
											playlist.status = Status::DraggingVolume(cursor.y);
											shell.capture_event();
											return;
										}
									}
								}

								if cursor.y - clip_bounds.y > LINE_HEIGHT + 12.0 {
									break 'block;
								}

								let fade_start_tab_dist =
									(clip_bounds.x + fade_start_px + 4.0 - cursor.x).abs();
								let fade_end_tab_dist = (clip_bounds.x
									+ layout.bounds().width + fade_end_px
									- 4.0 - cursor.x)
									.abs();

								let left_of_start_tab = clip_bounds.x + fade_start_px > cursor.x;
								let left_of_end_tab =
									clip_bounds.x + layout.bounds().width + fade_end_px > cursor.x;

								let use_start =
									match (fade_start_tab_dist <= 6.0, fade_end_tab_dist <= 6.0) {
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
							Inner::Recording(..) => {}
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
							cursor.y - clip_bounds.y.max(0.0) < LINE_HEIGHT,
						) {
							(false, false, false, false, _) => Status::Dragging(index.0, time),
							(false, _, true, false, _) => Status::TrimmingStart(time),
							(false, _, false, true, _) => Status::TrimmingEnd(time),
							(true, false, _, _, _) => {
								clear = false;
								let time = maybe_snap(time, *modifiers, |time| {
									time.round(beats_snap_step(playlist.scale, self.transport))
								});
								Status::Selecting(index.0, index.0, time, time)
							}
							(false, true, _, _, _) => {
								shell.publish((self.f)(Action::Clone));
								Status::Dragging(index.0, time)
							}
							(true, true, _, _, false) => Status::DraggingSlip(time),
							(true, true, _, _, true) => {
								let time = maybe_snap(time, *modifiers, |time| {
									time.round(beats_snap_step(playlist.scale, self.transport))
								});
								shell.publish((self.f)(Action::SplitAt(time)));
								Status::DraggingSplit(time)
							}
							(_, _, true, true, _) => unreachable!(),
						};

						shell.capture_event();
						shell.request_redraw();
					}
					mouse::Button::Right if playlist.status != Status::Deleting => {
						clear = true;
						playlist.status = Status::Deleting;
						shell.publish((self.f)(Action::Delete));
						shell.capture_event();
					}
					_ => {}
				}

				if clear {
					playlist.primary.clear();
					playlist.primary.insert(index);
				}
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

		let mut upper_bounds = bounds;
		upper_bounds.height = upper_bounds.height.min(LINE_HEIGHT);

		let color = match &self.inner {
			Inner::AudioClip(AudioClipRef { .. }) | Inner::MidiClip(MidiClipRef { .. }) => {
				match (state.enabled, state.selected) {
					(true, true) => theme.palette().danger.weak.color,
					(true, false) => theme.palette().primary.weak.color,
					(false, true) => theme.palette().secondary.strong.color,
					(false, false) => theme.palette().secondary.weak.color,
				}
			}
			Inner::Recording(..) => theme.palette().warning.weak.color,
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
				Inner::Recording(inner) => &*inner.name,
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
				hint_factor: renderer.scale_factor(),
			};

			renderer.fill_text(
				clip_name,
				upper_bounds.position()
					+ Vector::new(
						3.0,
						if upper_bounds.y == viewport.y {
							upper_bounds.height - LINE_HEIGHT / 2.0
						} else {
							LINE_HEIGHT / 2.0
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
		let samples_per_px = samples_per_px(playlist.scale, self.transport);

		match self.inner {
			Inner::AudioClip(inner) => {
				let unclipped_bounds = layout.bounds().shrink(padding::top(LINE_HEIGHT));

				if mesh_cache.is_empty()
					&& let Some(mesh) = debug::time_with("Waveform Mesh", || {
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
							samples_per_px / resample_ratio as f32 * inner.clip.stretch as f32,
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

					let fade_start_px = inner.clip.fade_start.len.to_samples(self.transport) as f32
						/ samples_per_px;
					let fade_end_px =
						inner.clip.fade_end.len.to_samples(self.transport) as f32 / -samples_per_px;

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

					if fade_start_px > 0.0 {
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

					if fade_end_px < 0.0 {
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
							let control = Point::new(lower_bounds.width / 2.0, lower_bounds.height);

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

					let mut content = format_db(inner.clip.volume.abs());

					if inner.clip.volume.is_sign_negative() {
						content += " (i)";
					}

					if state.show_controls || content != "0.0 dB" {
						let text = Text {
							content: &*content,
							bounds: Size::INFINITE,
							size: renderer.default_size(),
							line_height: 1.0.into(),
							font: renderer.default_font(),
							align_x: Alignment::Center,
							align_y: Vertical::Bottom,
							shaping: Shaping::Auto,
							wrapping: Wrapping::None,
							ellipsis: Ellipsis::None,
							hint_factor: None,
						};

						let size = Paragraph::with_text(text).min_bounds().expand((4.0, 4.0));

						if lower_bounds.width >= size.width
							&& lower_bounds.height >= size.height + 5.0
						{
							let control =
								Point::new(lower_bounds.width / 2.0, lower_bounds.height - 6.0);

							frame.fill(
								&Path::rounded_rectangle(
									control - Vector::new(size.width / 2.0, size.height - 2.0),
									size,
									2.into(),
								),
								color.scale_alpha(ALPHA_2_3),
							);

							frame.fill_text(canvas::Text {
								content,
								position: control,
								max_width: f32::INFINITY,
								color: theme.palette().background.strong.text,
								size: renderer.default_size(),
								line_height: 1.0.into(),
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
							|frame| fill_canvas(renderer, frame),
						));
					},
				);
			}
			Inner::MidiClip(inner) => 'blk: {
				if lower_bounds.width < 1.0 || inner.pattern.notes.is_empty() {
					break 'blk;
				}

				let (min, max) = inner
					.pattern
					.notes
					.iter()
					.fold((255, 0), |(min, max), note| {
						(note.key.0.min(min), note.key.0.max(max))
					});

				let note_height = (layout.bounds().height - LINE_HEIGHT) / f32::from(max - min + 3);
				let offset = Vector::new(layout.position().x, layout.position().y + LINE_HEIGHT);

				for note in &inner.pattern.notes {
					let start_pixel = note
						.position
						.start()
						.saturating_sub(inner.clip.position.offset())
						.to_samples(self.transport) as f32
						/ samples_per_px;
					let end_pixel = note
						.position
						.end()
						.saturating_sub(inner.clip.position.offset())
						.to_samples(self.transport) as f32
						/ samples_per_px;

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
			Inner::Recording(inner) => {
				if mesh_cache.is_empty()
					&& let Some(mesh) = debug::time_with("Waveform Mesh", || {
						let resample_ratio = inner.core.resample_ratio(self.transport);
						inner.lods.mesh(
							inner.core.samples(),
							self.transport,
							samples_per_px / resample_ratio as f32,
							theme.palette().background.strong.text,
							layout.bounds().shrink(padding::top(LINE_HEIGHT)),
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

		let playlist = self.playlist.borrow();

		match self.inner {
			Inner::AudioClip(inner) => 'block: {
				if cursor.y - (viewport.y - layout.position().y).max(0.0) < LINE_HEIGHT {
					break 'block;
				}

				let samples_per_px = samples_per_px(playlist.scale, self.transport);
				let fade_start_px =
					inner.clip.fade_start.len.to_samples(self.transport) as f32 / samples_per_px;
				let fade_end_px =
					inner.clip.fade_end.len.to_samples(self.transport) as f32 / -samples_per_px;

				let fade_start_control = Point::new(
					inner.clip.fade_start.p.x * fade_start_px,
					(1.0 - inner.clip.fade_start.p.y) * (layout.bounds().height - LINE_HEIGHT)
						+ LINE_HEIGHT,
				);

				let fade_end_control = Point::new(
					layout.bounds().width + inner.clip.fade_end.p.x * fade_end_px,
					(1.0 - inner.clip.fade_end.p.y) * (layout.bounds().height - LINE_HEIGHT)
						+ LINE_HEIGHT,
				);

				if fade_start_px >= 8.0 && cursor.distance(fade_start_control) <= 5.0
					|| fade_end_px <= -8.0 && cursor.distance(fade_end_control) <= 5.0
				{
					return Interaction::Crosshair;
				}

				let bounds = layout.bounds().intersection(viewport).unwrap()
					- Vector::new(layout.position().x, layout.position().y);
				let volume_control =
					bounds.position() + Vector::new(bounds.width / 2.0, bounds.height);
				if bounds.width >= 8.0 && cursor.distance(volume_control) <= 10.0 {
					return Interaction::ResizingVertically;
				}

				if cursor.y > LINE_HEIGHT + 12.0 {
					break 'block;
				}

				if (fade_start_px + 4.0 - cursor.x).abs() <= 6.0
					|| (layout.bounds().width + fade_end_px - 4.0 - cursor.x).abs() <= 6.0
				{
					return Interaction::Pointer;
				}
			}
			Inner::MidiClip(..) | Inner::Recording(..) => {}
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
			Inner::Recording(..) => Interaction::NotAllowed,
		}
	}
}

impl<'a, Message> Clip<'a, Message> {
	pub fn new(
		inner: impl Into<Inner<'a>>,
		playlist: &'a RefCell<playlist::State>,
		transport: &'a Transport,
		enabled: bool,
		f: fn(Action) -> Message,
	) -> Self {
		Self {
			inner: inner.into(),
			playlist,
			transport,
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
