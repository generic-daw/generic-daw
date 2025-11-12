use iced::{
	Element, Font,
	font::Family,
	padding,
	widget::{container, text},
};

pub static LUCIDE_BYTES: &[u8] = include_bytes!("../../Lucide.ttf");
pub static LUCIDE_FONT: Font = Font {
	family: Family::Name("lucide"),
	..Font::MONOSPACE
};

#[derive(Clone, Copy, Debug)]
pub struct Icon {
	character: char,
	size: f32,
}

impl Icon {
	pub fn size(mut self, size: f32) -> Self {
		self.size = size;
		self
	}
}

impl<'a, Message: 'a> From<Icon> for Element<'a, Message> {
	fn from(value: Icon) -> Self {
		container(
			text(value.character)
				.font(LUCIDE_FONT)
				.shaping(text::Shaping::Basic)
				.line_height(1.0)
				.size(value.size)
				.width(value.size)
				.center(),
		)
		.padding(padding::top(0.025 * value.size).bottom(-0.025 * value.size))
		.into()
	}
}

macro_rules! icon {
	($name:ident = $icon:literal) => {
		pub const fn $name() -> Icon {
			Icon {
				character: char::from_u32($icon).unwrap(),
				size: $crate::widget::LINE_HEIGHT,
			}
		}
	};
}

// https://unpkg.com/lucide-static@latest/font/info.json
icon!(chevron_down = 57453);
icon!(chevron_right = 57455);
icon!(chevron_up = 57456);
icon!(file = 57536);
icon!(grip_vertical = 57579);
icon!(link = 57602);
icon!(mic = 57624);
icon!(pause = 57646);
icon!(play = 57660);
icon!(plus = 57661);
icon!(power = 57664);
icon!(rotate_ccw = 57672);
icon!(save = 57677);
icon!(sliders_vertical = 57698);
icon!(square = 57703);
icon!(unlink = 57756);
icon!(volume_2 = 57771);
icon!(x = 57778);
icon!(move_vertical = 57799);
icon!(power_off = 57865);
icon!(arrow_left_right = 57930);
icon!(arrow_up_down = 58237);
icon!(chart_no_axes_gantt = 58564);
icon!(radius = 58669);
icon!(file_music = 58718);
