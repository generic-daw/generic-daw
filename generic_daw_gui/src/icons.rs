include!(concat!(env!("OUT_DIR"), "/icons.rs"));

use iced::{
	Element, padding,
	widget::{container, text},
};

#[derive(Clone, Copy, Debug)]
pub struct Icon {
	glyph: char,
	size: f32,
}

impl Icon {
	pub const fn size(mut self, size: f32) -> Self {
		self.size = size;
		self
	}

	pub const fn glyph(self) -> char {
		self.glyph
	}
}

impl<'a, Message: 'a> From<Icon> for Element<'a, Message> {
	fn from(value: Icon) -> Self {
		container(
			text(value.glyph)
				.font(LUCIDE_FONT)
				.shaping(text::Shaping::Basic)
				.size(value.size)
				.line_height(1.0),
		)
		.padding(padding::top(0.045 * value.size).bottom(-0.045 * value.size))
		.center(value.size)
		.into()
	}
}
