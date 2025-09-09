pub static LUCIDE_BYTES: &[u8] = include_bytes!("../../Lucide.ttf");
pub static LUCIDE_FONT: iced::Font = iced::Font::with_name("lucide");

macro_rules! icon {
	($name:ident = $icon:literal) => {
		pub fn $name<'a>() -> ::iced::widget::Text<'a> {
			::iced::widget::text(const { ::core::char::from_u32($icon).unwrap() })
				.font(LUCIDE_FONT)
				.shaping(::iced::widget::text::Shaping::Basic)
				.size(crate::widget::LINE_HEIGHT)
				.line_height(1.0)
		}
	};
}

// https://unpkg.com/lucide-static@latest/font/info.json
icon!(chevron_down = 57453);
icon!(chevron_right = 57455);
icon!(chevron_up = 57456);
icon!(file = 57536);
icon!(grip_vertical = 57579);
icon!(mic = 57624);
icon!(pause = 57646);
icon!(play = 57660);
icon!(plus = 57661);
icon!(rotate_ccw = 57672);
icon!(save = 57677);
icon!(sliders_vertical = 57698);
icon!(square = 57703);
icon!(volume_2 = 57771);
icon!(x = 57778);
icon!(move_vertical = 57799);
icon!(chart_no_axes_gantt = 58568);
icon!(file_music = 58722);
