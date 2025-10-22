use super::{LINE_HEIGHT, Vec2};
use crate::arrangement_view::{AudioClipRef, Recording as RecordingWrapper};
use generic_daw_core::{ClipPosition, MusicalTime, NotePosition, RtState};
use iced::{
	Element, Event, Fill, Length, Rectangle, Renderer, Shrink, Size, Theme, Vector,
	advanced::{
		Clipboard, Layout, Renderer as _, Shell, Text, Widget,
		graphics::{Mesh, mesh::Renderer as _},
		layout::{Limits, Node},
		renderer::{Quad, Style},
		text::Renderer as _,
		widget::{Tree, tree},
	},
	alignment::Vertical,
	border, debug,
	mouse::{Cursor, Interaction},
	padding,
	widget::text::{Alignment, LineHeight, Shaping, Wrapping},
	window,
};
use std::cell::RefCell;

#[derive(Default)]
struct State {
	cache: RefCell<Option<Mesh>>,
	last_bounds: Rectangle,
	last_addr: usize,
}

#[derive(Clone, Debug)]
pub enum Inner<'a> {
	Sample(AudioClipRef<'a>),
	Recording(&'a RecordingWrapper),
}

impl<'a> From<AudioClipRef<'a>> for Inner<'a> {
	fn from(value: AudioClipRef<'a>) -> Self {
		Self::Sample(value)
	}
}

impl<'a> From<&'a RecordingWrapper> for Inner<'a> {
	fn from(value: &'a RecordingWrapper) -> Self {
		Self::Recording(value)
	}
}

#[derive(Clone, Debug)]
pub struct AudioClip<'a> {
	inner: Inner<'a>,
	rtstate: &'a RtState,
	position: &'a Vec2,
	scale: &'a Vec2,
	enabled: bool,
}

impl<Message> Widget<Message, Theme, Renderer> for AudioClip<'_> {
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
			Inner::Sample(inner) => std::ptr::from_ref(inner.clip).addr(),
			Inner::Recording(inner) => std::ptr::from_ref(inner).addr(),
		};

		if state.last_addr != addr {
			*state = State::default();
			state.last_addr = addr;
		}
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
		let (start, len) = match self.inner {
			Inner::Sample(inner) => {
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

		let pixel_size = self.scale.x.exp2();
		Node::new(Size::new(len / pixel_size, limits.max().height))
			.translate(Vector::new((start - self.position.x) / pixel_size, 0.0))
	}

	fn update(
		&mut self,
		tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		_cursor: Cursor,
		_renderer: &Renderer,
		_clipboard: &mut dyn Clipboard,
		_shell: &mut Shell<'_, Message>,
		viewport: &Rectangle,
	) {
		if let Event::Window(window::Event::RedrawRequested(..)) = event
			&& let Some(mut bounds) = layout.bounds().intersection(viewport)
		{
			bounds.x = layout.position().x;
			bounds.y = 0.0;

			let state = tree.state.downcast_mut::<State>();
			if state.last_bounds != bounds {
				state.last_bounds = bounds;
				*state.cache.get_mut() = None;
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
			Inner::Sample(..) => {
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
			Inner::Sample(inner) => inner.sample.name.as_ref(),
			Inner::Recording(inner) => inner.name.as_ref(),
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

		let mesh = || match self.inner {
			Inner::Sample(inner) => inner.sample.lods.mesh(
				&inner.sample.samples,
				self.rtstate,
				inner.clip.position,
				self.position.x,
				self.scale.x,
				theme,
				lower_bounds.size(),
				layout.bounds().height - LINE_HEIGHT,
				layout.position().y - bounds.y,
			),
			Inner::Recording(inner) => inner.lods.mesh(
				inner.core.samples(),
				self.rtstate,
				ClipPosition::new(
					NotePosition::new(
						inner.position,
						inner.position
							+ MusicalTime::from_samples(inner.core.samples().len(), self.rtstate)
								.max(MusicalTime::TICK),
					),
					MusicalTime::ZERO,
				),
				self.position.x,
				self.scale.x,
				theme,
				lower_bounds.size(),
				layout.bounds().height - LINE_HEIGHT,
				layout.position().y - bounds.y,
			),
		};

		if state.cache.borrow().is_none()
			&& let Some(mesh) = debug::time_with("Waveform Mesh", mesh)
		{
			state.cache.borrow_mut().replace(mesh);
		}

		if let Some(mesh) = state.cache.borrow().clone() {
			renderer.with_translation(Vector::new(lower_bounds.x, lower_bounds.y), |renderer| {
				renderer.draw_mesh(mesh);
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
		let Some(bounds) = layout.bounds().intersection(viewport) else {
			return Interaction::default();
		};

		let Some(cursor) = cursor.position_in(bounds) else {
			return Interaction::default();
		};

		match self.inner {
			Inner::Sample(..) => match (cursor.x < 10.0, bounds.width - cursor.x < 10.0) {
				(true, true) => {
					match (
						cursor.x < bounds.width / 3.0,
						bounds.width - cursor.x < bounds.width / 3.0,
					) {
						(false, false) => Interaction::Grab,
						(true, false) | (false, true) => Interaction::ResizingHorizontally,
						(true, true) => unreachable!(),
					}
				}
				(true, false) | (false, true) => Interaction::ResizingHorizontally,
				(false, false) => Interaction::Grab,
			},
			Inner::Recording(..) => Interaction::NoDrop,
		}
	}
}

impl<'a> AudioClip<'a> {
	pub fn new(
		inner: impl Into<Inner<'a>>,
		rtstate: &'a RtState,
		position: &'a Vec2,
		scale: &'a Vec2,
		enabled: bool,
	) -> Self {
		Self {
			inner: inner.into(),
			rtstate,
			position,
			scale,
			enabled,
		}
	}
}

impl<'a, Message> From<AudioClip<'a>> for Element<'a, Message>
where
	Message: 'a,
{
	fn from(value: AudioClip<'a>) -> Self {
		Self::new(value)
	}
}
