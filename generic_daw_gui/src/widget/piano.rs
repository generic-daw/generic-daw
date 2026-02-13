use crate::widget::{LINE_HEIGHT, key_y};
use generic_daw_core::MidiKey;
use iced::{
	Color, Element, Length, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Layout, Renderer as _, Text, Widget,
		layout::{Limits, Node},
		renderer::{Quad, Style},
		text::Renderer as _,
		widget::Tree,
	},
	alignment::Vertical,
	mouse::Cursor,
	widget::text::{Alignment, LineHeight, Shaping, Wrapping},
};

const PIANO_WIDTH: f32 = 2.5 * LINE_HEIGHT;

#[derive(Clone, Copy, Debug)]
pub struct Piano<'a> {
	position: &'a Vector,
	scale: &'a Vector,
}

impl<Message> Widget<Message, Theme, Renderer> for Piano<'_> {
	fn size(&self) -> Size<Length> {
		Size::new(
			Length::Fixed(PIANO_WIDTH),
			Length::Fixed(128.0 * self.scale.y),
		)
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
		Node::new(Size::new(PIANO_WIDTH, 128.0 * self.scale.y))
	}

	fn draw(
		&self,
		_tree: &Tree,
		renderer: &mut Renderer,
		_theme: &Theme,
		_style: &Style,
		layout: Layout<'_>,
		_cursor: Cursor,
		viewport: &Rectangle,
	) {
		let Some(bounds) = layout.bounds().intersection(viewport) else {
			return;
		};

		renderer.fill_quad(
			Quad {
				bounds,
				..Quad::default()
			},
			Color::WHITE,
		);

		for key in (0..128).map(MidiKey) {
			let note_position =
				bounds.position() + Vector::new(0.0, key_y(key, *self.position, *self.scale));

			let Some(clipped_bounds) = Rectangle::new(
				note_position,
				Size::new(PIANO_WIDTH - self.scale.y / 2.0, self.scale.y),
			)
			.intersection(&bounds) else {
				continue;
			};

			if key.is_black() {
				renderer.fill_quad(
					Quad {
						bounds: clipped_bounds,
						..Quad::default()
					},
					Color::BLACK,
				);
			} else {
				let note_name = Text {
					content: key.to_string(),
					bounds: Size::new(f32::INFINITY, 0.0),
					size: renderer.default_size(),
					line_height: LineHeight::default(),
					font: renderer.default_font(),
					align_x: Alignment::Right,
					align_y: Vertical::Center,
					shaping: Shaping::Basic,
					wrapping: Wrapping::None,
					hint_factor: renderer.scale_factor(),
				};

				renderer.fill_text(
					note_name,
					note_position + Vector::new(PIANO_WIDTH - 2.0, self.scale.y / 2.0),
					Color::BLACK,
					Rectangle::INFINITE,
				);
			}
		}
	}
}

impl<'a> Piano<'a> {
	pub fn new(position: &'a Vector, scale: &'a Vector) -> Self {
		Self { position, scale }
	}
}

impl<'a, Message> From<Piano<'a>> for Element<'a, Message> {
	fn from(value: Piano<'a>) -> Self {
		Element::new(value)
	}
}
