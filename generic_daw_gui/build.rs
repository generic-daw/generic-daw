use std::{collections::BTreeSet, fs::File, io::Write as _, path::PathBuf};

static LUCIDE_BYTES: &[u8] = include_bytes!("../Lucide.ttf");

macro_rules! icon {
	($name:ident = $icon:literal) => {
		icon!($name = $icon + 0.05)
	};
	($name:ident = $icon:literal + $offset:literal) => {
		(
			stringify!($name),
			const { char::from_u32($icon).unwrap() },
			$offset,
		)
	};
}

// https://unpkg.com/lucide-static@latest/font/codepoints.json
static GLYPHS: &[(&str, char, f32)] = &[
	icon!(chevron_down = 57453),
	icon!(chevron_right = 57455),
	icon!(chevron_up = 57456),
	icon!(chevrons_down = 57457),
	icon!(copy = 57502),
	icon!(cpu = 57513),
	icon!(file = 57536),
	icon!(gavel = 57568),
	icon!(grip_horizontal = 57578),
	icon!(grip_vertical = 57579),
	icon!(pause = 57646),
	icon!(play = 57660),
	icon!(plus = 57661 + 0.025),
	icon!(power = 57664),
	icon!(rotate_ccw = 57672),
	icon!(save = 57677),
	icon!(sliders_vertical = 57698),
	icon!(snowflake = 57701),
	icon!(square = 57703),
	icon!(triangle_alert = 57747),
	icon!(x = 57778),
	icon!(move_vertical = 57799 + 0.025),
	icon!(arrow_big_right = 57827),
	icon!(power_off = 57865),
	icon!(folder_open = 57927),
	icon!(hourglass = 58006),
	icon!(file_headphone = 58138),
	icon!(file_play = 58145),
	icon!(circle_ellipsis = 58182),
	icon!(arrow_up_down = 58237),
	icon!(chart_no_axes_gantt = 58564),
	icon!(folder_sync = 58569),
	icon!(file_music = 58718),
	icon!(keyboard_music = 58720),
	icon!(between_horizontal_start = 58770),
	icon!(between_vertical_start = 58772),
	icon!(chevrons_left_right_ellipsis = 58911),
	icon!(metronome = 59068 + 0.025),
];

pub fn main() {
	println!("cargo::rerun-if-changed=../Lucide.ttf");

	let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
	let mut icons_rs = File::create(out_dir.join("icons.rs")).unwrap();

	icons_rs
		.write_all(
			br#"mod icons {

use crate::widget::LINE_HEIGHT;
use iced::{
	Element, Font, padding,
	widget::{container, text},
};

pub static LUCIDE_BYTES: &[u8] = include_bytes!("icons.ttf");
pub static LUCIDE_FONT: Font = Font::new("lucide");

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
"#,
		)
		.unwrap();

	let mut subset = BTreeSet::new();

	for &(name, glyph, offset) in GLYPHS {
		subset.insert(glyph);
		icons_rs
			.write_all(
				format!(
					"
pub const fn {name}() -> Icon {{
	Icon {{
		glyph: {glyph:?},
		size: LINE_HEIGHT,
		offset: {offset},
	}}
}}
"
				)
				.as_bytes(),
			)
			.unwrap();
	}

	icons_rs
		.write_all(
			b"
}",
		)
		.unwrap();

	std::fs::write(
		out_dir.join("icons.ttf"),
		font_subset::FontReader::new(LUCIDE_BYTES)
			.unwrap()
			.read()
			.unwrap()
			.subset(&subset)
			.unwrap()
			.to_opentype(),
	)
	.unwrap();
}
