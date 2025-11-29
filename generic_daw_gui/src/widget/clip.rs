use crate::{
	arrangement_view::{AudioClipRef, MidiClipRef, Recording as RecordingWrapper},
	widget::{
		LINE_HEIGHT, get_time, maybe_snap_time,
		playlist::{Action, Selection, Status},
	},
};
use generic_daw_core::{ClipPosition, MusicalTime, NotePosition, Transport};
use iced::{
	Event, Fill, Length, Point, Rectangle, Renderer, Shrink, Size, Theme, Vector,
	advanced::{
		Clipboard, Layout, Renderer as _, Shell, Text, Widget,
		graphics::mesh::{Cache, Renderer as _},
		layout::{Limits, Node},
		mouse::{self, Click, Cursor, Interaction, click::Kind},
		renderer::{Quad, Style},
		text::Renderer as _,
		widget::{Tree, tree},
	},
	alignment::Vertical,
	border, debug, padding,
	widget::text::{Alignment, LineHeight, Shaping, Wrapping},
	window,
};
use std::{borrow::Borrow, cell::RefCell, sync::Arc};

struct State {
	cache: RefCell<Cache>,
	last_click: Option<Click>,
	last_bounds: Rectangle,
	last_scale: Vector,
	last_addr: usize,
	last_theme: RefCell<Option<Theme>>,
}

impl Default for State {
	fn default() -> Self {
		Self {
			cache: RefCell::new(Cache::new(Arc::default())),
			last_click: None,
			last_bounds: Rectangle::default(),
			last_scale: Vector::default(),
			last_addr: 0,
			last_theme: RefCell::default(),
		}
	}
}

#[derive(Clone, Debug)]
pub enum Inner<'a> {
	AudioClip(AudioClipRef<'a>),
	MidiClip(MidiClipRef<'a>),
	Recording(&'a RecordingWrapper),
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

impl<'a> From<&'a RecordingWrapper> for Inner<'a> {
	fn from(value: &'a RecordingWrapper) -> Self {
		Self::Recording(value)
	}
}

#[derive(Clone, Debug)]
pub struct Clip<'a, Message> {
	pub(super) inner: Inner<'a>,
	selection: &'a RefCell<Selection>,
	transport: &'a Transport,
	position: &'a Vector,
	scale: &'a Vector,
	enabled: bool,
	f: fn(Action) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for Clip<'_, Message>
where
	Message: Clone,
{
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

		if state.last_scale != *self.scale {
			state.last_scale = *self.scale;
			if !state.cache.get_mut().is_empty() {
				state.cache.get_mut().update(Arc::default());
			}
		}
	}

	fn size(&self) -> Size<Length> {
		Size::new(Shrink, Fill)
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
		let (start, len) = match self.inner {
			Inner::AudioClip(inner) => {
				let start = inner.clip.position.start().to_samples_f(self.transport);
				let end = inner.clip.position.end().to_samples_f(self.transport);
				(start, end - start)
			}
			Inner::MidiClip(inner) => {
				let start = inner.clip.position.start().to_samples_f(self.transport);
				let end = inner.clip.position.end().to_samples_f(self.transport);
				(start, end - start)
			}
			Inner::Recording(inner) => {
				let start = inner.position.to_samples_f(self.transport);
				let len = inner.core.samples().len() as f32;
				(start, len)
			}
		};

		let samples_per_px = self.scale.x.exp2();
		Node::new(Size::new(len / samples_per_px, limits.max().height))
			.translate(Vector::new(start / samples_per_px, 0.0))
	}

	fn update(
		&mut self,
		tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		cursor: Cursor,
		_renderer: &Renderer,
		_clipboard: &mut dyn Clipboard,
		shell: &mut Shell<'_, Message>,
		viewport: &Rectangle,
	) {
		let state = tree.state.downcast_mut::<State>();

		if let Event::Window(window::Event::RedrawRequested(..)) = event
			&& let Some(mut bounds) = layout.bounds().intersection(viewport)
		{
			bounds.x = layout.position().x;
			bounds.y = layout.position().y - bounds.y;

			if state.last_bounds != bounds {
				state.last_bounds = bounds;
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

		let selection = &mut *self.selection.borrow_mut();
		let clip_bounds = layout.bounds() - Vector::new(viewport.x, viewport.y);

		if let Event::Mouse(event) = event
			&& clip_bounds.contains(cursor)
		{
			match event {
				mouse::Event::ButtonPressed { button, modifiers }
					if selection.status == Status::None =>
				{
					let mut clear = selection.primary.insert(idx);

					match button {
						mouse::Button::Left => {
							if matches!(self.inner, Inner::MidiClip(..)) {
								let new_click =
									Click::new(cursor, mouse::Button::Left, state.last_click);
								state.last_click = Some(new_click);

								if new_click.kind() == Kind::Double {
									selection.primary.clear();
									selection.primary.insert(idx);
									shell.publish((self.f)(Action::Open));
									shell.capture_event();
									return;
								}
							}

							let time =
								get_time(cursor.x, *self.position, *self.scale, self.transport);

							selection.status = match (modifiers.command(), modifiers.shift()) {
								(false, false) => {
									let start_pixel = clip_bounds.x;
									let end_pixel = clip_bounds.x + clip_bounds.width;
									let start_offset = cursor.x - start_pixel;
									let end_offset = end_pixel - cursor.x;
									let border = 10f32.min((end_pixel - start_pixel) / 3.0);
									match (start_offset < border, end_offset < border) {
										(true, false) => Status::TrimmingStart(time),
										(false, true) => Status::TrimmingEnd(time),
										(false, false) => Status::Dragging(idx.0, time),
										(true, true) => unreachable!(),
									}
								}
								(true, false) => {
									clear = false;
									let time = maybe_snap_time(time, *modifiers, |time| {
										time.snap_round(self.scale.x, self.transport)
									});
									Status::Selecting(idx.0, idx.0, time, time)
								}
								(false, true) => {
									shell.publish((self.f)(Action::Clone));
									Status::Dragging(idx.0, time)
								}
								(true, true) => {
									let time = maybe_snap_time(time, *modifiers, |time| {
										time.snap_round(self.scale.x, self.transport)
									});
									shell.publish((self.f)(Action::SplitAt(time)));
									Status::DraggingSplit(time)
								}
							};

							shell.capture_event();
							shell.request_redraw();
						}
						mouse::Button::Right if selection.status != Status::Deleting => {
							selection.status = Status::Deleting;
							shell.publish((self.f)(Action::Delete));
							shell.capture_event();
						}
						_ => {}
					}

					if clear {
						selection.primary.clear();
						selection.primary.insert(idx);
					}
				}
				mouse::Event::CursorMoved { .. } if selection.status == Status::Deleting => {
					selection.primary.insert(idx);
				}
				_ => {}
			}
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

		let mut upper_bounds = bounds;
		upper_bounds.height = upper_bounds.height.min(LINE_HEIGHT);

		let selection = self.selection.borrow();

		let color = match &self.inner {
			Inner::AudioClip(AudioClipRef { idx, .. })
			| Inner::MidiClip(MidiClipRef { idx, .. }) => {
				match (
					self.enabled,
					selection.primary.contains(idx) || selection.secondary.contains(idx),
				) {
					(true, true) => theme.extended_palette().danger.weak.color,
					(true, false) => theme.extended_palette().primary.weak.color,
					(false, true) => theme.extended_palette().secondary.strong.color,
					(false, false) => theme.extended_palette().secondary.weak.color,
				}
			}
			Inner::Recording(..) => theme.extended_palette().warning.weak.color,
		};

		let text_background = Quad {
			bounds: upper_bounds,
			..Quad::default()
		};
		renderer.fill_quad(text_background, color);

		let name = match self.inner {
			Inner::AudioClip(inner) => &inner.sample.name,
			Inner::MidiClip(..) => "MIDI Clip",
			Inner::Recording(inner) => &inner.name,
		};

		let text = Text {
			content: name.into(),
			bounds: Size::new(f32::INFINITY, 0.0),
			size: renderer.default_size(),
			line_height: LineHeight::default(),
			font: renderer.default_font(),
			align_x: Alignment::Left,
			align_y: Vertical::Top,
			shaping: Shaping::Auto,
			wrapping: Wrapping::None,
		};
		renderer.fill_text(
			text,
			upper_bounds.position() + Vector::new(3.0, 0.0),
			theme.extended_palette().background.strong.text,
			upper_bounds,
		);

		if bounds.height == upper_bounds.height {
			return;
		}

		let lower_bounds = bounds.shrink(padding::top(upper_bounds.height));

		let clip_background = Quad {
			bounds: lower_bounds,
			border: border::width(1).color(color),
			..Quad::default()
		};
		renderer.fill_quad(clip_background, color.scale_alpha(0.2));

		let state = tree.state.downcast_ref::<State>();
		let cache = &mut *state.cache.borrow_mut();
		let last_theme = &mut *state.last_theme.borrow_mut();

		if last_theme.as_ref() != Some(theme) {
			*last_theme = Some(theme.clone());
			cache.update(Arc::default());
		}

		let unclipped_height = layout.bounds().height - LINE_HEIGHT;
		let hidden_top_px = layout.position().y - bounds.y;

		match self.inner {
			Inner::AudioClip(inner) => {
				if cache.is_empty()
					&& let Some(mesh) = debug::time_with("Waveform Mesh", || {
						inner.sample.lods.mesh(
							&inner.sample.samples,
							self.transport,
							inner.clip.position,
							self.position.x,
							self.scale.x,
							theme,
							lower_bounds.size(),
							unclipped_height,
							hidden_top_px,
						)
					}) {
					cache.update(Arc::from([mesh]));
				}
			}
			Inner::MidiClip(inner) => 'blk: {
				debug_assert!(cache.is_empty());

				let (min, max) = inner
					.pattern
					.notes
					.iter()
					.fold((255, 0), |(min, max), note| {
						(note.key.0.min(min), note.key.0.max(max))
					});

				if min > max {
					break 'blk;
				}

				let samples_per_px = self.scale.x.exp2();
				let note_height = unclipped_height / f32::from(max - min + 3);
				let offset = Vector::new(layout.position().x, layout.position().y + LINE_HEIGHT);

				for note in &inner.pattern.notes {
					let start_pixel = (note
						.position
						.start()
						.saturating_sub(inner.clip.position.offset())
						.to_samples_f(self.transport))
						/ samples_per_px;
					let end_pixel = (note
						.position
						.end()
						.saturating_sub(inner.clip.position.offset())
						.to_samples_f(self.transport))
						/ samples_per_px;

					let top_pixel = f32::from(max - note.key.0 + 1) * note_height;

					let note_bounds = Rectangle::new(
						Point::new(start_pixel, top_pixel) + offset,
						Size::new(end_pixel - start_pixel, note_height),
					);

					let Some(bounds) = note_bounds.intersection(&lower_bounds) else {
						continue;
					};

					renderer.fill_quad(
						Quad {
							bounds,
							..Quad::default()
						},
						theme.extended_palette().background.strong.text,
					);
				}
			}
			Inner::Recording(inner) => {
				if cache.is_empty()
					&& let Some(mesh) = debug::time_with("Waveform Mesh", || {
						let clip_position = ClipPosition::new(
							NotePosition::new(
								inner.position,
								inner.position
									+ MusicalTime::from_samples(
										inner.core.samples().len(),
										self.transport,
									)
									.max(MusicalTime::TICK),
							),
							MusicalTime::ZERO,
						);

						inner.lods.mesh(
							inner.core.samples(),
							self.transport,
							clip_position,
							self.position.x,
							self.scale.x,
							theme,
							lower_bounds.size(),
							unclipped_height,
							hidden_top_px,
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
		selection: &'a RefCell<Selection>,
		transport: &'a Transport,
		position: &'a Vector,
		scale: &'a Vector,
		enabled: bool,
		f: fn(Action) -> Message,
	) -> Self {
		Self {
			inner: inner.into(),
			selection,
			transport,
			position,
			scale,
			enabled,
			f,
		}
	}
}

impl<'a, Message> Borrow<dyn Widget<Message, Theme, Renderer> + 'a> for Clip<'a, Message>
where
	Message: Clone + 'a,
{
	fn borrow(&self) -> &(dyn Widget<Message, Theme, Renderer> + 'a) {
		self
	}
}

impl<'a, Message> Borrow<dyn Widget<Message, Theme, Renderer> + 'a> for &Clip<'a, Message>
where
	Message: Clone + 'a,
{
	fn borrow(&self) -> &(dyn Widget<Message, Theme, Renderer> + 'a) {
		*self
	}
}
