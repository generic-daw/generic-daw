use crate::{
	arrangement_view::{AudioClipRef, MidiClipRef, Recording},
	widget::{
		ALPHA_1_3, LINE_HEIGHT, beats_snap_step, maybe_snap,
		playlist::{self, Action, Status},
		px_to_time, samples_per_px, time_to_px,
	},
};
use generic_daw_core::{Transport, time::BeatTime};
use iced::{
	Event, Length, Point, Rectangle, Renderer, Shrink, Size, Theme, Vector,
	advanced::{
		Layout, Renderer as _, Shell, Text, Widget,
		graphics::mesh::{Cache, Renderer as _},
		layout::{Limits, Node},
		mouse::{self, Click, Cursor, Interaction, click::Kind},
		renderer::{Quad, Style},
		text::Renderer as _,
		widget::{Tree, tree},
	},
	alignment::Vertical,
	border, debug, padding,
	widget::text::{Alignment, Ellipsis, LineHeight, Shaping, Wrapping},
	window,
};
use std::{borrow::Borrow, cell::RefCell, sync::Arc};

struct State {
	cache: RefCell<Cache>,
	last_click: Option<Click>,
	last_bounds: Rectangle,
	last_offset: BeatTime,
	last_stretch: f32,
	last_addr: usize,
	last_theme: RefCell<Option<Theme>>,
}

impl Default for State {
	fn default() -> Self {
		Self {
			cache: RefCell::new(Cache::new(Arc::default())),
			last_click: None,
			last_bounds: Rectangle::default(),
			last_stretch: 0.0,
			last_offset: BeatTime::ZERO,
			last_addr: 0,
			last_theme: RefCell::default(),
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

	fn diff(&self, tree: &mut Tree) {
		let state = tree.state.downcast_mut::<State>();

		let addr = match self.inner {
			Inner::AudioClip(inner) => std::ptr::from_ref(inner.sample).addr(),
			Inner::MidiClip(inner) => std::ptr::from_ref(inner.pattern).addr(),
			Inner::Recording(inner) => std::ptr::from_ref(inner).addr(),
		};

		if state.last_addr != addr {
			state.last_addr = addr;
			if !state.cache.get_mut().is_empty() {
				state.cache.get_mut().update(Arc::default());
			}
		}
	}

	fn size(&self) -> Size<Length> {
		Size::new(Shrink, Shrink)
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
		let playlist = &mut *self.playlist.borrow_mut();

		if let Event::Window(window::Event::RedrawRequested(..)) = event {
			if let Some(mut bounds) = layout.bounds().intersection(viewport) {
				bounds.x -= layout.bounds().x;
				bounds.y -= layout.bounds().y;

				if state.last_bounds != bounds {
					state.last_bounds = bounds;
					if !state.cache.get_mut().is_empty() {
						state.cache.get_mut().update(Arc::default());
					}
				}
			}

			let offset = match self.inner {
				Inner::AudioClip(inner) => {
					inner.clip.position.offset().to_beat_time(self.transport)
				}
				Inner::MidiClip(inner) => inner.clip.position.offset(),
				Inner::Recording(..) => BeatTime::ZERO,
			};

			if state.last_offset != offset {
				state.last_offset = offset;
				if !state.cache.get_mut().is_empty() {
					state.cache.get_mut().update(Arc::default());
				}
			}

			let stretch = samples_per_px(playlist.scale, self.transport)
				* match self.inner {
					Inner::AudioClip(inner) => inner.clip.stretch as f32,
					Inner::MidiClip(..) | Inner::Recording(..) => 1.0,
				};

			if state.last_stretch != stretch {
				state.last_stretch = stretch;
				if !state.cache.get_mut().is_empty() {
					state.cache.get_mut().update(Arc::default());
				}
			}
		}

		if shell.is_event_captured() {
			return;
		}

		let (Inner::AudioClip(AudioClipRef { idx, .. }) | Inner::MidiClip(MidiClipRef { idx, .. })) =
			self.inner
		else {
			return;
		};

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
				let mut clear = playlist.primary.insert(idx);

				match button {
					mouse::Button::Left => {
						if let Inner::MidiClip(..) = self.inner {
							let new_click =
								Click::new(cursor, mouse::Button::Left, state.last_click);
							state.last_click = Some(new_click);

							if new_click.kind() == Kind::Double {
								shell.publish((self.f)(Action::Open(idx.0, idx.1)));
								shell.capture_event();
								return;
							}
						}

						let time =
							px_to_time(cursor.x, playlist.position, playlist.scale, self.transport);

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
							(false, false, false, false, _) => Status::Dragging(idx.0, time),
							(false, _, true, false, _) => Status::TrimmingStart(time),
							(false, _, false, true, _) => Status::TrimmingEnd(time),
							(true, false, _, _, _) => {
								clear = false;
								let time = maybe_snap(time, *modifiers, |time| {
									time.round(beats_snap_step(playlist.scale, self.transport))
								});
								Status::Selecting(idx.0, idx.0, time, time)
							}
							(false, true, _, _, _) => {
								shell.publish((self.f)(Action::Clone));
								Status::Dragging(idx.0, time)
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
					playlist.primary.insert(idx);
				}
			}
			Event::Mouse(mouse::Event::CursorMoved { .. })
				if playlist.status == Status::Deleting =>
			{
				playlist.primary.insert(idx);
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

		let playlist = self.playlist.borrow();

		let mut upper_bounds = bounds;
		upper_bounds.height = upper_bounds.height.min(LINE_HEIGHT);

		let color = match &self.inner {
			Inner::AudioClip(AudioClipRef { idx, .. })
			| Inner::MidiClip(MidiClipRef { idx, .. }) => {
				match (
					self.enabled,
					playlist.primary.contains(idx) || playlist.secondary.contains(idx),
				) {
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

		let state = tree.state.downcast_ref::<State>();
		let cache = &mut *state.cache.borrow_mut();
		let last_theme = &mut *state.last_theme.borrow_mut();

		if last_theme.as_ref() != Some(theme) {
			*last_theme = Some(theme.clone());
			if !cache.is_empty() {
				cache.update(Arc::default());
			}
		}

		let samples_per_px = samples_per_px(playlist.scale, self.transport);

		match self.inner {
			Inner::AudioClip(inner) => {
				if cache.is_empty()
					&& let Some(mesh) = debug::time_with("Waveform Mesh", || {
						let resample_ratio = inner.sample.resample_ratio(self.transport).recip();
						let stretch = inner.clip.stretch * resample_ratio;

						inner.sample.lods.mesh(
							&inner.sample.samples,
							(inner.clip.position.offset() * resample_ratio)
								.to_samples(self.transport),
							samples_per_px * stretch as f32,
							theme.palette().background.strong.text,
							layout.bounds().shrink(padding::top(LINE_HEIGHT)),
							lower_bounds,
						)
					}) {
					cache.update(Arc::from([mesh]));
				}
			}
			Inner::MidiClip(inner) => 'blk: {
				debug_assert!(cache.is_empty());

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
				if cache.is_empty()
					&& let Some(mesh) = debug::time_with("Waveform Mesh", || {
						inner.lods.mesh(
							inner.core.samples(),
							0,
							samples_per_px / inner.core.resample_ratio(self.transport) as f32,
							theme.palette().background.strong.text,
							layout.bounds().shrink(padding::top(LINE_HEIGHT)),
							lower_bounds,
						)
					}) {
					cache.update(Arc::from([mesh]));
				}
			}
		}

		renderer.with_translation(Vector::new(lower_bounds.x, lower_bounds.y), |renderer| {
			renderer.draw_mesh_cache(cache.clone());
		});
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

impl<'a, Message: 'a> Borrow<dyn Widget<Message, Theme, Renderer> + 'a> for &Clip<'a, Message> {
	fn borrow(&self) -> &(dyn Widget<Message, Theme, Renderer> + 'a) {
		*self
	}
}
