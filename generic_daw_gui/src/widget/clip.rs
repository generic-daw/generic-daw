use crate::{
	arrangement_view::{AudioClipRef, MidiClipRef, Recording as RecordingWrapper},
	widget::LINE_HEIGHT,
};
use generic_daw_core::{ClipPosition, MusicalTime, NotePosition, RtState};
use generic_daw_utils::Vec2;
use iced::{
	Event, Fill, Length, Point, Rectangle, Renderer, Shrink, Size, Theme, Vector,
	advanced::{
		Clipboard, Layout, Renderer as _, Shell, Text, Widget,
		graphics::{Mesh, mesh::Renderer as _},
		layout::{Limits, Node},
		mouse::{Click, click::Kind},
		renderer::{Quad, Style},
		text::Renderer as _,
		widget::{Tree, tree},
	},
	alignment::Vertical,
	border, debug,
	mouse::{self, Cursor, Interaction},
	padding,
	widget::text::{Alignment, LineHeight, Shaping, Wrapping},
	window,
};
use std::{
	borrow::{Borrow, BorrowMut},
	cell::RefCell,
};

#[derive(Default)]
struct State {
	cache: RefCell<Option<Mesh>>,
	last_click: Option<Click>,
	last_bounds: Rectangle,
	last_scale: Vec2,
	last_addr: usize,
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
	inner: Inner<'a>,
	rtstate: &'a RtState,
	position: &'a Vec2,
	scale: &'a Vec2,
	enabled: bool,
	on_double_click: Option<Message>,
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

	fn size(&self) -> Size<Length> {
		Size::new(Shrink, Fill)
	}

	fn diff(&self, tree: &mut Tree) {
		let state = tree.state.downcast_mut::<State>();

		let addr = match self.inner {
			Inner::AudioClip(inner) => std::ptr::from_ref(inner.sample).addr(),
			Inner::MidiClip(inner) => std::ptr::from_ref(inner.pattern).addr(),
			Inner::Recording(inner) => std::ptr::from_ref(inner).addr(),
		};

		if state.last_addr != addr {
			*state.cache.get_mut() = None;
			state.last_addr = addr;
		}

		if state.last_scale != *self.scale {
			*state.cache.get_mut() = None;
			state.last_scale = *self.scale;
		}
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
		let (start, len) = match self.inner {
			Inner::AudioClip(inner) => {
				let start = inner.clip.position.start().to_samples_f(self.rtstate);
				let end = inner.clip.position.end().to_samples_f(self.rtstate);
				(start, end - start)
			}
			Inner::MidiClip(inner) => {
				let start = inner.clip.position.start().to_samples_f(self.rtstate);
				let end = inner.clip.position.end().to_samples_f(self.rtstate);
				(start, end - start)
			}
			Inner::Recording(inner) => {
				let start = inner.position.to_samples_f(self.rtstate);
				let len = inner.core.samples().len() as f32;
				(start, len)
			}
		};

		let samples_per_px = self.scale.x.exp2();
		Node::new(Size::new(len / samples_per_px, limits.max().height))
			.translate(Vector::new((start - self.position.x) / samples_per_px, 0.0))
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
		if let Event::Window(window::Event::RedrawRequested(..)) = event
			&& let Some(mut bounds) = layout.bounds().intersection(viewport)
		{
			bounds.x = layout.position().x;
			bounds.y = layout.position().y - bounds.y;

			let state = tree.state.downcast_mut::<State>();
			if state.last_bounds != bounds {
				*state.cache.get_mut() = None;
				state.last_bounds = bounds;
			}
		}

		if shell.is_event_captured() {
			return;
		}

		if let Event::Mouse(mouse::Event::ButtonPressed {
			button: mouse::Button::Left,
			..
		}) = event && let Some(cursor) = cursor.position_in(layout.bounds())
			&& let Some(on_double_click) = &self.on_double_click
		{
			let state = tree.state.downcast_mut::<State>();

			let new_click = Click::new(cursor, mouse::Button::Left, state.last_click);
			state.last_click = Some(new_click);

			if new_click.kind() == Kind::Double {
				shell.publish(on_double_click.clone());
				shell.capture_event();
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

		let color = match self.inner {
			Inner::AudioClip(..) | Inner::MidiClip(..) => {
				if self.enabled {
					theme.extended_palette().primary.weak.color
				} else {
					theme.extended_palette().secondary.weak.color
				}
			}
			Inner::Recording(..) => theme.extended_palette().danger.weak.color,
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

		let cache = &mut *tree.state.downcast_ref::<State>().cache.borrow_mut();

		let unclipped_height = layout.bounds().height - LINE_HEIGHT;
		let hidden_top_px = layout.position().y - bounds.y;

		match self.inner {
			Inner::AudioClip(inner) => 'blk: {
				if cache.is_some() {
					break 'blk;
				}

				*cache = debug::time_with("Waveform Mesh", || {
					inner.sample.lods.mesh(
						&inner.sample.samples,
						self.rtstate,
						inner.clip.position,
						self.position.x,
						self.scale.x,
						theme,
						lower_bounds.size(),
						unclipped_height,
						hidden_top_px,
					)
				});
			}
			Inner::MidiClip(inner) => 'blk: {
				debug_assert!(cache.is_none());

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
						.to_samples_f(self.rtstate))
						/ samples_per_px;
					let end_pixel = (note
						.position
						.end()
						.saturating_sub(inner.clip.position.offset())
						.to_samples_f(self.rtstate))
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
			Inner::Recording(inner) => 'blk: {
				if cache.is_some() {
					break 'blk;
				}

				let clip_position = ClipPosition::new(
					NotePosition::new(
						inner.position,
						inner.position
							+ MusicalTime::from_samples(inner.core.samples().len(), self.rtstate)
								.max(MusicalTime::TICK),
					),
					MusicalTime::ZERO,
				);

				*cache = debug::time_with("Waveform Mesh", || {
					inner.lods.mesh(
						inner.core.samples(),
						self.rtstate,
						clip_position,
						self.position.x,
						self.scale.x,
						theme,
						lower_bounds.size(),
						unclipped_height,
						hidden_top_px,
					)
				});
			}
		}

		if let Some(mesh) = cache {
			renderer.with_translation(Vector::new(lower_bounds.x, lower_bounds.y), |renderer| {
				renderer.draw_mesh(mesh.clone());
			});
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

		match self.inner {
			Inner::AudioClip(..) | Inner::MidiClip(..) => {
				let border = 10f32.min(layout.bounds().width / 3.0);
				match (cursor.x < border, layout.bounds().width - cursor.x < border) {
					(false, false) => Interaction::Grab,
					(true, false) | (false, true) => Interaction::ResizingHorizontally,
					(true, true) => unreachable!(),
				}
			}
			Inner::Recording(..) => Interaction::NoDrop,
		}
	}
}

impl<'a, Message> Clip<'a, Message> {
	pub fn new(
		inner: impl Into<Inner<'a>>,
		rtstate: &'a RtState,
		position: &'a Vec2,
		scale: &'a Vec2,
		enabled: bool,
		on_double_click: Option<Message>,
	) -> Self {
		Self {
			inner: inner.into(),
			rtstate,
			position,
			scale,
			enabled,
			on_double_click,
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

impl<'a, Message> BorrowMut<dyn Widget<Message, Theme, Renderer> + 'a> for Clip<'a, Message>
where
	Message: Clone + 'a,
{
	fn borrow_mut(&mut self) -> &mut (dyn Widget<Message, Theme, Renderer> + 'a) {
		self
	}
}
