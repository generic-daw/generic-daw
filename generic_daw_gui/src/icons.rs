pub static LUCIDE_BYTES: &[u8] = include_bytes!("../../Lucide.ttf");
pub static LUCIDE_FONT: iced::Font = iced::Font::with_name("lucide");

macro_rules! icon {
	($name:ident = $icon:literal) => {
		pub fn $name<'a>() -> ::iced::widget::Text<'a> {
			::iced::widget::text(const { ::core::char::from_u32($icon).unwrap() })
				.font(LUCIDE_FONT)
				.shaping(::iced::widget::text::Shaping::Advanced)
				.size(crate::widget::LINE_HEIGHT)
				.line_height(1.0)
		}
	};
}

// https://unpkg.com/lucide-static@latest/font/info.json
icon!(chevron_down = 57457);
icon!(chevron_right = 57459);
icon!(chevron_up = 57460);
icon!(file = 57540);
icon!(grip_vertical = 57583);
icon!(mic = 57628);
icon!(pause = 57650);
icon!(play = 57664);
icon!(plus = 57665);
icon!(rotate_ccw = 57676);
icon!(save = 57681);
icon!(sliders_vertical = 57702);
icon!(square = 57707);
icon!(volume_2 = 57771);
icon!(x = 57778);
icon!(move_vertical = 57799);
icon!(chart_no_axes_gantt = 58569);
icon!(file_music = 58723);
