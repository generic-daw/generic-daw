// automatically generated

use crate::widget::LINE_HEIGHT;
use iced::{
	Element, Font, font, padding,
	widget::{container, text},
};

pub static LUCIDE_BYTES: &[u8] = include_bytes!("../../icons.ttf");
pub static LUCIDE_FONT: Font = Font {
	family: font::Family::Name("lucide"),
	..Font::MONOSPACE
};

#[derive(Clone, Copy, Debug)]
pub struct Icon {
	glyph: char,
	size: f32,
	offset: f32,
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
				.line_height(1.0)
				.size(value.size)
				.width(value.size)
				.center(),
		)
		.padding(padding::top(value.offset * value.size).bottom(-value.offset * value.size))
		.into()
	}
}

pub const fn chevron_down() -> Icon {
	Icon {
		glyph: '\u{e06d}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn chevron_right() -> Icon {
	Icon {
		glyph: '\u{e06f}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn chevron_up() -> Icon {
	Icon {
		glyph: '\u{e070}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn cpu() -> Icon {
	Icon {
		glyph: '\u{e0a9}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn file() -> Icon {
	Icon {
		glyph: '\u{e0c0}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn grip_vertical() -> Icon {
	Icon {
		glyph: '\u{e0eb}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn link() -> Icon {
	Icon {
		glyph: '\u{e102}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn mic() -> Icon {
	Icon {
		glyph: '\u{e118}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn pause() -> Icon {
	Icon {
		glyph: '\u{e12e}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn play() -> Icon {
	Icon {
		glyph: '\u{e13c}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn plus() -> Icon {
	Icon {
		glyph: '\u{e13d}',
		size: LINE_HEIGHT,
		offset: 0.025,
	}
}

pub const fn power() -> Icon {
	Icon {
		glyph: '\u{e140}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn rotate_ccw() -> Icon {
	Icon {
		glyph: '\u{e148}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn save() -> Icon {
	Icon {
		glyph: '\u{e14d}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn sliders_vertical() -> Icon {
	Icon {
		glyph: '\u{e162}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn square() -> Icon {
	Icon {
		glyph: '\u{e167}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn unlink() -> Icon {
	Icon {
		glyph: '\u{e19c}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn volume_2() -> Icon {
	Icon {
		glyph: '\u{e1ab}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn x() -> Icon {
	Icon {
		glyph: '\u{e1b2}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn move_vertical() -> Icon {
	Icon {
		glyph: '\u{e1c7}',
		size: LINE_HEIGHT,
		offset: 0.025,
	}
}

pub const fn power_off() -> Icon {
	Icon {
		glyph: '\u{e209}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn arrow_left_right() -> Icon {
	Icon {
		glyph: '\u{e24a}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn file_headphone() -> Icon {
	Icon {
		glyph: '\u{e31a}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn file_play() -> Icon {
	Icon {
		glyph: '\u{e321}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn arrow_up_down() -> Icon {
	Icon {
		glyph: '\u{e37d}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn chart_no_axes_gantt() -> Icon {
	Icon {
		glyph: '\u{e4c4}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn radius() -> Icon {
	Icon {
		glyph: '\u{e52d}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn file_music() -> Icon {
	Icon {
		glyph: '\u{e55e}',
		size: LINE_HEIGHT,
		offset: 0.05,
	}
}

pub const fn metronome() -> Icon {
	Icon {
		glyph: '\u{e6bc}',
		size: LINE_HEIGHT,
		offset: 0.025,
	}
}
