use iced::Color;

pub trait ColorExt {
    fn mix(self, other: Self, amount: f32) -> Self;
}

impl ColorExt for Color {
    fn mix(self, other: Self, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let self_amount = 1.0 - amount;

        let self_linear = self.into_linear().map(|c| c * self_amount);
        let other_linear = other.into_linear().map(|c| c * amount);

        Self::from_linear_rgba(
            self_linear[0] + other_linear[0],
            self_linear[1] + other_linear[1],
            self_linear[2] + other_linear[2],
            self_linear[3] + other_linear[3],
        )
    }
}
