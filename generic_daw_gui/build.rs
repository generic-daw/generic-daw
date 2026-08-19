use std::{collections::BTreeSet, fs::File, io::Write as _, path::PathBuf};

static LUCIDE_BYTES: &[u8] = include_bytes!("../Lucide.ttf");

macro_rules! icon {
	($name:ident = $icon:literal) => {
		(stringify!($name), const { char::from_u32($icon).unwrap() })
	};
}

// https://unpkg.com/lucide-static@latest/font/codepoints.json
static GLYPHS: &[(&str, char)] = &[
	icon!(chevron_down = 57453),
	icon!(chevron_right = 57455),
	icon!(chevron_up = 57456),
	icon!(copy = 57502),
	icon!(cpu = 57513),
	icon!(file = 57536),
	icon!(gavel = 57568),
	icon!(grip_horizontal = 57578),
	icon!(grip_vertical = 57579),
	icon!(menu = 57621),
	icon!(mic = 57624),
	icon!(pause = 57646),
	icon!(play = 57660),
	icon!(plus = 57661),
	icon!(power = 57664),
	icon!(rotate_ccw = 57672),
	icon!(save = 57677),
	icon!(sliders_vertical = 57698),
	icon!(snowflake = 57701),
	icon!(square = 57703),
	icon!(triangle_alert = 57747),
	icon!(volume_2 = 57771),
	icon!(x = 57778),
	icon!(move_vertical = 57799),
	icon!(arrow_big_right = 57827),
	icon!(power_off = 57865),
	icon!(folder_open = 57927),
	icon!(hourglass = 58006),
	icon!(magnet = 58037),
	icon!(file_headphone = 58138),
	icon!(file_play = 58145),
	icon!(circle_ellipsis = 58182),
	icon!(arrow_up_down = 58237),
	icon!(replace = 58331),
	icon!(panel_bottom_dashed = 58414),
	icon!(chart_no_axes_gantt = 58564),
	icon!(folder_sync = 58569),
	icon!(file_music = 58718),
	icon!(keyboard_music = 58720),
	icon!(between_horizontal_start = 58770),
	icon!(between_vertical_start = 58772),
	icon!(chevrons_left_right_ellipsis = 58911),
	icon!(metronome = 59068),
	icon!(square_arrow_right_enter = 59075),
	icon!(midi_port = 59193),
];

pub fn main() {
	println!("cargo::rerun-if-changed=../Lucide.ttf");

	let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
	let mut icons_rs = File::create(out_dir.join("icons.rs")).unwrap();

	icons_rs
		.write_all(
			br#"
pub static LUCIDE_BYTES: &[u8] = include_bytes!("icons.ttf");
pub static LUCIDE_FONT: iced::Font = iced::Font::new("lucide");
"#,
		)
		.unwrap();

	let mut subset = BTreeSet::new();

	for &(name, glyph) in GLYPHS {
		subset.insert(glyph);
		icons_rs
			.write_all(
				format!(
					"
pub const fn {name}() -> Icon {{
	Icon {{
		glyph: {glyph:?},
		size: crate::widget::LINE_HEIGHT,
	}}
}}
"
				)
				.as_bytes(),
			)
			.unwrap();
	}

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
