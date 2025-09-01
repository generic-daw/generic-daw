use iced_widget::core::Color;

pub mod dot;
pub mod drag_handle;
pub mod knob;
pub mod peak_meter;

fn mix(a: Color, b: Color, factor: f32) -> Color {
	let b_amount = factor.clamp(0.0, 1.0);
	let a_amount = 1.0 - b_amount;

	let a_linear = a.into_linear().map(|c| c * a_amount);
	let b_linear = b.into_linear().map(|c| c * b_amount);

	Color::from_linear_rgba(
		a_linear[0] + b_linear[0],
		a_linear[1] + b_linear[1],
		a_linear[2] + b_linear[2],
		a_linear[3] + b_linear[3],
	)
}
