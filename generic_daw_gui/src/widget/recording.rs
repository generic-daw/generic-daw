use super::{LINE_HEIGHT, Vec2, waveform};
use crate::arrangement_view::Recording as RecordingWrapper;
use generic_daw_core::{MusicalTime, RtState};
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
	border,
	mouse::Cursor,
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
pub struct Recording<'a> {
	inner: &'a RecordingWrapper,
	rtstate: &'a RtState,
	position: &'a Vec2,
	scale: &'a Vec2,
}

impl<Message> Widget<Message, Theme, Renderer> for Recording<'_> {
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

		if state.last_addr != std::ptr::from_ref(self.inner).addr() {
			*state = State::default();
			state.last_addr = std::ptr::from_ref(self.inner).addr();
		}
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
		let start = self.inner.position.to_samples_f(self.rtstate);
		let len = 2.0 * self.inner.lods[0].len() as f32;
		let pixel_size = self.scale.x.exp2();

		Node::new(Size::new(len / pixel_size, self.scale.y))
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
		if let Event::Window(window::Event::RedrawRequested(..)) = event {
			let mut bounds = layout.bounds();
			if let Some(intersection) = bounds.intersection(viewport) {
				bounds.y = 0.0;
				bounds.height = intersection.height;

				let state = tree.state.downcast_mut::<State>();
				if state.last_bounds != bounds {
					state.last_bounds = bounds;
					*state.cache.get_mut() = None;
				}
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

		let color = theme.extended_palette().danger.weak.color;

		let text_background = Quad {
			bounds: upper_bounds,
			..Quad::default()
		};
		renderer.fill_quad(text_background, color);

		let text = Text {
			content: self.inner.name.as_ref().into(),
			bounds: Size::new(f32::INFINITY, 0.0),
			size: renderer.default_size(),
			line_height: LineHeight::default(),
			font: renderer.default_font(),
			align_x: Alignment::Left,
			align_y: Vertical::Top,
			shaping: Shaping::Basic,
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

		if state.cache.borrow().is_none()
			&& let Some(mesh) = waveform::mesh(
				self.rtstate,
				self.inner.position,
				MusicalTime::ZERO,
				&self.inner.lods,
				*self.position,
				*self.scale,
				theme,
				layout.position().y,
				lower_bounds,
			) {
			state.cache.borrow_mut().replace(mesh);
		}

		if let Some(mesh) = state.cache.borrow().clone() {
			renderer.with_translation(Vector::new(bounds.x, layout.position().y), |renderer| {
				renderer.draw_mesh(mesh);
			});
		}
	}
}

impl<'a> Recording<'a> {
	pub fn new(
		inner: &'a RecordingWrapper,
		rtstate: &'a RtState,
		position: &'a Vec2,
		scale: &'a Vec2,
	) -> Self {
		Self {
			inner,
			rtstate,
			position,
			scale,
		}
	}
}

impl<'a, Message> From<Recording<'a>> for Element<'a, Message>
where
	Message: 'a,
{
	fn from(value: Recording<'a>) -> Self {
		Self::new(value)
	}
}
